use hmac::{Hmac, Mac};
use nanoid::nanoid;
use sha2::{Digest, Sha256};

pub const PUBLISHER_PREFIX: &str = "hld_pub_";
pub const SUBSCRIBER_PREFIX: &str = "hld_sub_";

pub fn generate_api_key(prefix: &str) -> (String, String, String) {
    let raw = format!("{}{}", prefix, nanoid!(24));
    let hash = hash_api_key(&raw);
    let key_prefix = raw.chars().take(12).collect::<String>();
    (raw, hash, key_prefix)
}

pub fn hash_api_key(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Sign a payload with HMAC-SHA256.
/// 
/// Note: new_from_slice only fails for algorithms with key length constraints.
/// SHA256 accepts any key length, so this is infallible in practice.
pub fn sign_payload(secret: &str, timestamp: i64, body: &str) -> String {
    let data = format!("{}.{}", timestamp, body);
    // HMAC-SHA256 accepts any key length, so this cannot fail
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC-SHA256 accepts any key length");
    mac.update(data.as_bytes());
    format!("sha256={:x}", mac.finalize().into_bytes())
}

pub fn verify_signature(secret: &str, timestamp: i64, body: &str, signature: &str) -> bool {
    let expected = sign_payload(secret, timestamp, body);
    subtle::ConstantTimeEq::ct_eq(expected.as_bytes(), signature.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_publisher_api_key() {
        let (raw, hash, prefix) = generate_api_key(PUBLISHER_PREFIX);
        
        assert!(raw.starts_with("hld_pub_"), "key should start with publisher prefix");
        assert_eq!(raw.len(), 8 + 24, "key should be prefix(8) + nanoid(24)");
        assert_eq!(prefix.len(), 12, "prefix should be 12 chars");
        assert!(raw.starts_with(&prefix), "raw key should start with stored prefix");
        assert!(!hash.is_empty(), "hash should not be empty");
        assert_eq!(hash.len(), 64, "SHA256 hash should be 64 hex chars");
    }

    #[test]
    fn test_generate_subscriber_api_key() {
        let (raw, hash, prefix) = generate_api_key(SUBSCRIBER_PREFIX);
        
        assert!(raw.starts_with("hld_sub_"), "key should start with subscriber prefix");
        assert_eq!(raw.len(), 8 + 24);
        assert_eq!(prefix.len(), 12);
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_hash_api_key_deterministic() {
        let key = "hld_pub_test123456789012345678";
        let hash1 = hash_api_key(key);
        let hash2 = hash_api_key(key);
        
        assert_eq!(hash1, hash2, "hashing same key should produce same result");
    }

    #[test]
    fn test_hash_api_key_different_inputs() {
        let hash1 = hash_api_key("key_a");
        let hash2 = hash_api_key("key_b");
        
        assert_ne!(hash1, hash2, "different keys should produce different hashes");
    }

    #[test]
    fn test_sign_payload_format() {
        let signature = sign_payload("secret", 1707379800, r#"{"event":"signal"}"#);
        
        assert!(signature.starts_with("sha256="), "signature should have sha256= prefix");
        assert_eq!(signature.len(), 7 + 64, "signature should be prefix(7) + hex(64)");
    }

    #[test]
    fn test_sign_payload_deterministic() {
        let sig1 = sign_payload("secret", 1707379800, "body");
        let sig2 = sign_payload("secret", 1707379800, "body");
        
        assert_eq!(sig1, sig2, "same inputs should produce same signature");
    }

    #[test]
    fn test_sign_payload_different_secrets() {
        let sig1 = sign_payload("secret1", 1707379800, "body");
        let sig2 = sign_payload("secret2", 1707379800, "body");
        
        assert_ne!(sig1, sig2, "different secrets should produce different signatures");
    }

    #[test]
    fn test_sign_payload_different_timestamps() {
        let sig1 = sign_payload("secret", 1707379800, "body");
        let sig2 = sign_payload("secret", 1707379801, "body");
        
        assert_ne!(sig1, sig2, "different timestamps should produce different signatures");
    }

    #[test]
    fn test_verify_signature_valid() {
        let secret = "webhook_secret";
        let timestamp = 1707379800;
        let body = r#"{"event":"signal","channel_id":"ch_123"}"#;
        let signature = sign_payload(secret, timestamp, body);
        
        assert!(
            verify_signature(secret, timestamp, body, &signature),
            "valid signature should verify"
        );
    }

    #[test]
    fn test_verify_signature_wrong_secret() {
        let timestamp = 1707379800;
        let body = "body";
        let signature = sign_payload("secret1", timestamp, body);
        
        assert!(
            !verify_signature("secret2", timestamp, body, &signature),
            "wrong secret should fail verification"
        );
    }

    #[test]
    fn test_verify_signature_wrong_timestamp() {
        let secret = "secret";
        let body = "body";
        let signature = sign_payload(secret, 1707379800, body);
        
        assert!(
            !verify_signature(secret, 1707379801, body, &signature),
            "wrong timestamp should fail verification"
        );
    }

    #[test]
    fn test_verify_signature_tampered_body() {
        let secret = "secret";
        let timestamp = 1707379800;
        let signature = sign_payload(secret, timestamp, "original body");
        
        assert!(
            !verify_signature(secret, timestamp, "tampered body", &signature),
            "tampered body should fail verification"
        );
    }

    #[test]
    fn test_verify_signature_malformed() {
        assert!(
            !verify_signature("secret", 1707379800, "body", "not_a_valid_signature"),
            "malformed signature should fail"
        );
        assert!(
            !verify_signature("secret", 1707379800, "body", "sha256=invalid"),
            "invalid hex should fail"
        );
    }

    #[test]
    fn test_api_key_uniqueness() {
        let (key1, _, _) = generate_api_key(PUBLISHER_PREFIX);
        let (key2, _, _) = generate_api_key(PUBLISHER_PREFIX);
        
        assert_ne!(key1, key2, "generated keys should be unique");
    }
}
