use actix_web::{HttpResponse, Responder, get};
use serde_json::json;

#[get("/health")]
pub async fn health() -> impl Responder {
    HttpResponse::Ok().json(json!({
        "status"  :"ok",
        "message"  :"Healthy"
    }))
}
