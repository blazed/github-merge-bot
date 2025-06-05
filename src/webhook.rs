// webhook.rs
use axum::http::HeaderMap;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
pub struct WebhookHandler {
    secret: String,
}

impl WebhookHandler {
    pub fn new(secret: &str) -> Self {
        Self {
            secret: secret.to_string(),
        }
    }

    pub async fn verify_signature(&self, headers: &HeaderMap, body: &str) -> bool {
        let signature = match headers.get("X-Hub-Signature-256") {
            Some(sig) => match sig.to_str() {
                Ok(s) => s,
                Err(_) => return false,
            },
            None => return false,
        };

        if !signature.starts_with("sha256=") {
            return false;
        }

        let expected_signature = &signature[7..]; // Remove "sha256=" prefix

        let mut mac = match HmacSha256::new_from_slice(self.secret.as_bytes()) {
            Ok(mac) => mac,
            Err(_) => return false,
        };

        mac.update(body.as_bytes());
        let result = mac.finalize();
        let computed_signature = hex::encode(result.into_bytes());

        // Constant-time comparison
        computed_signature == expected_signature
    }
}
