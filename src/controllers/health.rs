use actix_web::{HttpResponse, Responder, get};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde_json::json;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[get("/health")]
pub async fn health() -> impl Responder {
    let version = std::env::var("MIN_PLUGIN_VERSION")
        .or_else(|_| std::env::var("APP_VERSION"))
        .unwrap_or_else(|_| "1.3.3".to_string());
    let healthy = true;
    let timestamp = Utc::now().timestamp();

    let secret = std::env::var("JWT_SECRET").unwrap_or_default();
    let signature = if !secret.is_empty() {
        let payload = format!("{healthy}:{version}:{timestamp}");
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(payload.as_bytes());
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    } else {
        String::new()
    };

    HttpResponse::Ok().json(json!({
        "status": "ok",
        "healthy": healthy,
        "version": version,
        "timestamp": timestamp,
        "signature": signature
    }))
}
