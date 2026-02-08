//! Tests for tunnel protocol and agent registry

#[cfg(test)]
mod tests {
    use super::super::tunnel::*;
    use super::super::types::SignalUrgency;
    use chrono::Utc;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    // ============================================================
    // Protocol Message Serialization Tests
    // ============================================================

    #[test]
    fn test_client_auth_message_serialization() {
        let msg = ClientMessage::Auth {
            token: "hld_sub_test123".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"auth\""));
        assert!(json.contains("\"token\":\"hld_sub_test123\""));

        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            ClientMessage::Auth { token } => assert_eq!(token, "hld_sub_test123"),
            _ => panic!("Expected Auth message"),
        }
    }

    #[test]
    fn test_client_ack_message_serialization() {
        let msg = ClientMessage::Ack {
            delivery_id: "del_xyz789".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"ack\""));
        assert!(json.contains("\"delivery_id\":\"del_xyz789\""));

        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            ClientMessage::Ack { delivery_id } => assert_eq!(delivery_id, "del_xyz789"),
            _ => panic!("Expected Ack message"),
        }
    }

    #[test]
    fn test_client_pong_message_serialization() {
        let msg = ClientMessage::Pong;
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"pong\""));

        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, ClientMessage::Pong));
    }

    #[test]
    fn test_server_auth_ok_message_serialization() {
        let msg = ServerMessage::AuthOk {
            connection_id: "conn_abc123".to_string(),
            subscriber_id: "sub_001".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"auth_ok\""));
        assert!(json.contains("\"connection_id\":\"conn_abc123\""));
        assert!(json.contains("\"subscriber_id\":\"sub_001\""));
    }

    #[test]
    fn test_server_auth_error_message_serialization() {
        let msg = ServerMessage::AuthError {
            message: "Invalid token".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"auth_error\""));
        assert!(json.contains("\"message\":\"Invalid token\""));
    }

    #[test]
    fn test_server_signal_message_serialization() {
        let msg = ServerMessage::Signal {
            delivery_id: "del_test123".to_string(),
            channel_id: "ch_abc".to_string(),
            channel_slug: "tech-news".to_string(),
            signal: TunnelSignal {
                id: "sig_xyz".to_string(),
                title: "Test Signal".to_string(),
                body: "This is a test".to_string(),
                urgency: SignalUrgency::High,
                metadata: serde_json::json!({"source": "test"}),
                created_at: Utc::now(),
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"signal\""));
        assert!(json.contains("\"channel_slug\":\"tech-news\""));
        assert!(json.contains("\"urgency\":\"high\""));
    }

    #[test]
    fn test_server_ping_message_serialization() {
        let msg = ServerMessage::Ping;
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"ping\""));
    }

    // ============================================================
    // Agent Registry Tests
    // ============================================================

    #[tokio::test]
    async fn test_registry_register_and_get() {
        let registry = AgentRegistry::new();
        let (tx, _rx) = mpsc::channel(10);

        let conn = AgentConnection {
            connection_id: "conn_test".to_string(),
            subscriber_id: "sub_001".to_string(),
            sender: tx,
            connected_at: Utc::now(),
        };

        registry.register(conn).await;

        let retrieved = registry.get("sub_001").await;
        assert!(retrieved.is_some());
        let agent = retrieved.unwrap();
        assert_eq!(agent.connection_id, "conn_test");
        assert_eq!(agent.subscriber_id, "sub_001");
    }

    #[tokio::test]
    async fn test_registry_unregister() {
        let registry = AgentRegistry::new();
        let (tx, _rx) = mpsc::channel(10);

        let conn = AgentConnection {
            connection_id: "conn_test".to_string(),
            subscriber_id: "sub_001".to_string(),
            sender: tx,
            connected_at: Utc::now(),
        };

        registry.register(conn).await;
        assert!(registry.get("sub_001").await.is_some());

        registry.unregister("sub_001").await;
        assert!(registry.get("sub_001").await.is_none());
    }

    #[tokio::test]
    async fn test_registry_get_nonexistent() {
        let registry = AgentRegistry::new();
        assert!(registry.get("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn test_registry_overwrite_connection() {
        let registry = AgentRegistry::new();
        let (tx1, _rx1) = mpsc::channel(10);
        let (tx2, _rx2) = mpsc::channel(10);

        // Register first connection
        let conn1 = AgentConnection {
            connection_id: "conn_first".to_string(),
            subscriber_id: "sub_001".to_string(),
            sender: tx1,
            connected_at: Utc::now(),
        };
        registry.register(conn1).await;

        // Register second connection with same subscriber_id
        let conn2 = AgentConnection {
            connection_id: "conn_second".to_string(),
            subscriber_id: "sub_001".to_string(),
            sender: tx2,
            connected_at: Utc::now(),
        };
        registry.register(conn2).await;

        // Should have the second connection
        let agent = registry.get("sub_001").await.unwrap();
        assert_eq!(agent.connection_id, "conn_second");
    }

    #[tokio::test]
    async fn test_registry_concurrent_access() {
        let registry = Arc::new(AgentRegistry::new());
        let mut handles = vec![];

        // Spawn multiple tasks that register agents
        for i in 0..10 {
            let reg = registry.clone();
            let handle = tokio::spawn(async move {
                let (tx, _rx) = mpsc::channel(10);
                let conn = AgentConnection {
                    connection_id: format!("conn_{}", i),
                    subscriber_id: format!("sub_{}", i),
                    sender: tx,
                    connected_at: Utc::now(),
                };
                reg.register(conn).await;
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        // Verify all agents are registered
        for i in 0..10 {
            let agent = registry.get(&format!("sub_{}", i)).await;
            assert!(agent.is_some(), "Agent sub_{} should exist", i);
        }
    }

    // ============================================================
    // TunnelSignal Tests
    // ============================================================

    #[test]
    fn test_tunnel_signal_all_urgency_levels() {
        for urgency in [
            SignalUrgency::Low,
            SignalUrgency::Normal,
            SignalUrgency::High,
            SignalUrgency::Critical,
        ] {
            let signal = TunnelSignal {
                id: "sig_test".to_string(),
                title: "Test".to_string(),
                body: "Body".to_string(),
                urgency: urgency.clone(),
                metadata: serde_json::json!({}),
                created_at: Utc::now(),
            };

            let json = serde_json::to_string(&signal).unwrap();
            let parsed: TunnelSignal = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.urgency, urgency);
        }
    }

    #[test]
    fn test_tunnel_signal_with_complex_metadata() {
        let metadata = serde_json::json!({
            "source": "https://example.com",
            "tags": ["ai", "news", "breaking"],
            "nested": {
                "key": "value",
                "number": 42
            }
        });

        let signal = TunnelSignal {
            id: "sig_meta".to_string(),
            title: "Complex Metadata".to_string(),
            body: "Testing metadata".to_string(),
            urgency: SignalUrgency::Normal,
            metadata: metadata.clone(),
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&signal).unwrap();
        let parsed: TunnelSignal = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.metadata["source"], "https://example.com");
        assert_eq!(parsed.metadata["tags"][0], "ai");
        assert_eq!(parsed.metadata["nested"]["number"], 42);
    }

    // ============================================================
    // Edge Case Tests
    // ============================================================

    #[test]
    fn test_invalid_json_deserialization_client_message() {
        let invalid_json = r#"{"type": "unknown_type"}"#;
        let result: Result<ClientMessage, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_json_deserialization_server_message() {
        let invalid_json = r#"{"type": "invalid_message_type"}"#;
        let result: Result<ServerMessage, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_malformed_json_deserialization() {
        let malformed = r#"{"type": "auth", "token": }"#;
        let result: Result<ClientMessage, _> = serde_json::from_str(malformed);
        assert!(result.is_err());
    }

    #[test]
    fn test_client_auth_empty_token() {
        let msg = ClientMessage::Auth {
            token: "".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            ClientMessage::Auth { token } => assert!(token.is_empty()),
            _ => panic!("Expected Auth message"),
        }
    }

    #[test]
    fn test_tunnel_signal_with_null_metadata() {
        let signal = TunnelSignal {
            id: "sig_null".to_string(),
            title: "Null Metadata".to_string(),
            body: "Testing null".to_string(),
            urgency: SignalUrgency::Normal,
            metadata: serde_json::Value::Null,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&signal).unwrap();
        let parsed: TunnelSignal = serde_json::from_str(&json).unwrap();
        assert!(parsed.metadata.is_null());
    }

    #[test]
    fn test_tunnel_signal_with_empty_strings() {
        let signal = TunnelSignal {
            id: "".to_string(),
            title: "".to_string(),
            body: "".to_string(),
            urgency: SignalUrgency::Low,
            metadata: serde_json::json!({}),
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&signal).unwrap();
        let parsed: TunnelSignal = serde_json::from_str(&json).unwrap();
        assert!(parsed.id.is_empty());
        assert!(parsed.title.is_empty());
        assert!(parsed.body.is_empty());
    }

    #[tokio::test]
    async fn test_registry_unregister_nonexistent() {
        let registry = AgentRegistry::new();
        // Should not panic when unregistering non-existent subscriber
        registry.unregister("nonexistent_subscriber").await;
        assert!(registry.get("nonexistent_subscriber").await.is_none());
    }

    #[test]
    fn test_server_auth_error_with_special_characters() {
        let msg = ServerMessage::AuthError {
            message: "Invalid token: \"test\" <script>alert(1)</script>".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            ServerMessage::AuthError { message } => {
                assert!(message.contains("<script>"));
            }
            _ => panic!("Expected AuthError message"),
        }
    }

    #[test]
    fn test_client_ack_empty_delivery_id() {
        let msg = ClientMessage::Ack {
            delivery_id: "".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            ClientMessage::Ack { delivery_id } => assert!(delivery_id.is_empty()),
            _ => panic!("Expected Ack message"),
        }
    }
}
