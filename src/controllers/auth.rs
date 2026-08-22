use actix_web::{web, HttpResponse, Responder, Scope};
use serde::Deserialize;

use crate::services::auth as auth_service;

pub fn auth_scope() -> Scope {
    web::scope("/auth").route("/login", web::post().to(login))
}

#[derive(Deserialize)]
struct LoginBody {
    uuid: String,
}

async fn login(body: web::Json<LoginBody>) -> impl Responder {
    let (status, json_body) = auth_service::login_service(&body.uuid).await;
    HttpResponse::build(status).json(json_body)
}

