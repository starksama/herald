use serde::Serialize;

use core::tunnel::TunnelSignal;

pub struct Forwarder {
    client: reqwest::Client,
    forward_url: String,
}

impl Forwarder {
    pub fn new(forward_url: String) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self { client, forward_url })
    }

    pub async fn deliver_signal(
        &self,
        delivery_id: &str,
        channel_id: &str,
        channel_slug: &str,
        signal: &TunnelSignal,
    ) -> anyhow::Result<()> {
        let payload = ForwardPayload {
            delivery_id,
            channel_id,
            channel_slug,
            signal,
        };

        let resp = self
            .client
            .post(&self.forward_url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("forward failed: HTTP {}", resp.status()))
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ForwardPayload<'a> {
    delivery_id: &'a str,
    channel_id: &'a str,
    channel_slug: &'a str,
    signal: &'a TunnelSignal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use core::types::SignalUrgency;

    #[test]
    fn test_forwarder_new_succeeds() {
        let forwarder = Forwarder::new("http://localhost:8080/webhook".to_string());
        assert!(forwarder.is_ok());
    }

    #[test]
    fn test_forwarder_new_with_various_urls() {
        // Valid URLs
        let urls = vec![
            "http://localhost:8080",
            "https://api.example.com/hooks",
            "http://127.0.0.1:3000/callback",
            "https://my-service.internal:9000/v1/signals",
        ];

        for url in urls {
            let result = Forwarder::new(url.to_string());
            assert!(result.is_ok(), "Should accept valid URL: {}", url);
        }
    }

    #[test]
    fn test_forward_payload_serialization() {
        let signal = TunnelSignal {
            id: "sig_test123".to_string(),
            title: "Test Signal".to_string(),
            body: "This is a test body".to_string(),
            urgency: SignalUrgency::High,
            metadata: serde_json::json!({"key": "value"}),
            created_at: Utc::now(),
        };

        let payload = ForwardPayload {
            delivery_id: "del_abc",
            channel_id: "ch_xyz",
            channel_slug: "tech-news",
            signal: &signal,
        };

        let json = serde_json::to_string(&payload).unwrap();

        // Verify camelCase field names
        assert!(json.contains("\"deliveryId\":\"del_abc\""));
        assert!(json.contains("\"channelId\":\"ch_xyz\""));
        assert!(json.contains("\"channelSlug\":\"tech-news\""));
        assert!(json.contains("\"signal\":{"));
    }

    #[test]
    fn test_forward_payload_with_empty_fields() {
        let signal = TunnelSignal {
            id: "".to_string(),
            title: "".to_string(),
            body: "".to_string(),
            urgency: SignalUrgency::Low,
            metadata: serde_json::json!(null),
            created_at: Utc::now(),
        };

        let payload = ForwardPayload {
            delivery_id: "",
            channel_id: "",
            channel_slug: "",
            signal: &signal,
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"deliveryId\":\"\""));
        assert!(json.contains("\"channelId\":\"\""));
        assert!(json.contains("\"channelSlug\":\"\""));
    }

    #[test]
    fn test_forward_payload_with_special_characters() {
        let signal = TunnelSignal {
            id: "sig_special".to_string(),
            title: "Alert: \"Breaking\" <News>".to_string(),
            body: "Line1\nLine2\tTabbed".to_string(),
            urgency: SignalUrgency::Critical,
            metadata: serde_json::json!({"emoji": "🚀", "quote": "He said \"hello\""}),
            created_at: Utc::now(),
        };

        let payload = ForwardPayload {
            delivery_id: "del_special",
            channel_id: "ch_test",
            channel_slug: "my-channel",
            signal: &signal,
        };

        let json = serde_json::to_string(&payload).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["signal"]["title"], "Alert: \"Breaking\" <News>");
        assert!(parsed["signal"]["body"].as_str().unwrap().contains("\n"));
    }

    #[test]
    fn test_forward_payload_with_complex_metadata() {
        let signal = TunnelSignal {
            id: "sig_meta".to_string(),
            title: "Complex".to_string(),
            body: "Testing nested metadata".to_string(),
            urgency: SignalUrgency::Normal,
            metadata: serde_json::json!({
                "array": [1, 2, 3],
                "nested": {
                    "deep": {
                        "value": true
                    }
                },
                "nullField": null
            }),
            created_at: Utc::now(),
        };

        let payload = ForwardPayload {
            delivery_id: "del_meta",
            channel_id: "ch_meta",
            channel_slug: "meta-test",
            signal: &signal,
        };

        let json = serde_json::to_string(&payload).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["signal"]["metadata"]["array"][0], 1);
        assert_eq!(parsed["signal"]["metadata"]["nested"]["deep"]["value"], true);
        assert!(parsed["signal"]["metadata"]["nullField"].is_null());
    }
}
