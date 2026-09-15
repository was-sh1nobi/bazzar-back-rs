use actix_web::{HttpResponse, Responder, get};
use chrono::Utc;
use serde_json::json;

#[get("/health")]
pub async fn health() -> impl Responder {
    let version = std::env::var("MIN_PLUGIN_VERSION")
        .or_else(|_| std::env::var("APP_VERSION"))
        .unwrap_or_else(|_| "1.4.0".to_string());
    let healthy = true;
    let timestamp = Utc::now().timestamp();

    // Plain status endpoint — deliberately no shared-secret signature.
    // The plugin is public, so there is no plugin<->backend verification;
    // auth is per-user JWT only.
    HttpResponse::Ok().json(json!({
        "status": "ok",
        "healthy": healthy,
        "version": version,
        "timestamp": timestamp
    }))
}
