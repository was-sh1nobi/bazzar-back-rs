//! HTTP plumbing for /auth: routes, request extraction, status mapping.
//! All business logic and error semantics live in the service layer.

use actix_web::{HttpResponse, Scope, web};
use sea_orm::DatabaseConnection;
use serde::Deserialize;

use crate::auth::Auth;
use crate::services::{auth as auth_service, error::ServiceError};

pub fn auth_scope() -> Scope {
    web::scope("/auth")
        .route("/register", web::post().to(register))
        .route("/login", web::post().to(login))
        .route("/status", web::get().to(status))
        .route("/change", web::post().to(change))
}

#[derive(Deserialize)]
struct RegisterBody {
    #[serde(alias = "playerUuid", alias = "playeruuid", alias = "uuid")]
    player_uuid: String,
    #[serde(alias = "playerName", alias = "player_name", alias = "username", alias = "name")]
    player_name: String,
    password: String,
}

#[derive(Deserialize)]
struct LoginBody {
    #[serde(alias = "playerUuid", alias = "playeruuid", alias = "player_uuid")]
    uuid: String,
    password: String,
}

#[derive(Deserialize)]
struct StatusQuery {
    player: Option<String>,
}

#[derive(Deserialize)]
struct ChangeBody {
    #[serde(alias = "oldPassword", alias = "old_password")]
    old_password: String,
    #[serde(alias = "newPassword", alias = "new_password")]
    new_password: String,
}

async fn register(
    db: web::Data<DatabaseConnection>,
    body: web::Json<RegisterBody>,
) -> Result<HttpResponse, ServiceError> {
    let out = auth_service::register_service(
        auth_service::RegisterInput {
            uuid: body.player_uuid.clone(),
            name: body.player_name.clone(),
            password: body.password.clone(),
        },
        &db,
    )
    .await?;
    Ok(HttpResponse::Created().json(out))
}

async fn login(
    db: web::Data<DatabaseConnection>,
    body: web::Json<LoginBody>,
) -> Result<HttpResponse, ServiceError> {
    let out = auth_service::login_service(
        auth_service::LoginInput {
            uuid: body.uuid.clone(),
            password: body.password.clone(),
        },
        &db,
    )
    .await?;
    Ok(HttpResponse::Ok().json(out))
}

async fn status(
    query: web::Query<StatusQuery>,
    db: web::Data<DatabaseConnection>,
) -> Result<HttpResponse, ServiceError> {
    let Some(player) = query.player.as_deref() else {
        return Err(ServiceError::MissingParam(
            "player query param is required".to_owned(),
        ));
    };
    let out = auth_service::status_service(player, &db).await?;
    Ok(HttpResponse::Ok().json(out))
}

// NOTE: `web::Json` must be the LAST extractor — it consumes the payload.
async fn change(
    db: web::Data<DatabaseConnection>,
    auth: Auth,
    body: web::Json<ChangeBody>,
) -> Result<HttpResponse, ServiceError> {
    let out = auth_service::change_service(
        auth_service::ChangeInput {
            uuid: auth.claims.sub,
            old_password: body.old_password.clone(),
            new_password: body.new_password.clone(),
        },
        &db,
    )
    .await?;
    Ok(HttpResponse::Ok().json(out))
}
