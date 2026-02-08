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
