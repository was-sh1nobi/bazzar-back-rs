use crate::controllers::items::post_items_add_controller;
use actix_web::{HttpResponse, Scope, web};
use serde_json::json;

pub fn list_scope() -> Scope {
    web::scope("/list").route("", web::post().to(post_items_add_controller))
}

/// GET / — welcome route per guide §10 #0.
pub async fn root_welcome() -> HttpResponse {
    // ponytail: returns welcome payload matching Node Fastify response
    HttpResponse::Ok().json(json!({
        "status": 200,
        "message": "Welcome to Rust"
    }))
}
