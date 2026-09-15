//! HTTP plumbing for /auth: routes, request extraction, status mapping.
//! All business logic and error semantics live in the service layer.

use std::time::Duration;

use actix_web::{HttpRequest, HttpResponse, Scope, web};
use sea_orm::DatabaseConnection;
use serde::Deserialize;

use crate::auth::{Auth, revoke_token};
use crate::services::{auth as auth_service, error::ServiceError};
use crate::utils::rate_limit::{
    RATE_LIMITER, check_ip_rate_limit, check_jwt_rate_limit, extract_client_fingerprint,
};

pub fn auth_scope() -> Scope {
    web::scope("/auth")
        .route("/register", web::post().to(register))
        .route("/login", web::post().to(login))
        .route("/logout", web::post().to(logout))
        .route("/status", web::get().to(status))
        .route("/change", web::post().to(change))
}

#[derive(Deserialize)]
struct RegisterBody {
    #[serde(alias = "playerUuid", alias = "playeruuid", alias = "uuid")]
    player_uuid: String,
    #[serde(
        alias = "playerName",
        alias = "player_name",
        alias = "username",
        alias = "name"
    )]
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
    req: HttpRequest,
    db: web::Data<DatabaseConnection>,
    body: web::Json<RegisterBody>,
) -> Result<HttpResponse, ServiceError> {
    check_ip_rate_limit(&req, "auth_register", 20, 60)?;
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
    req: HttpRequest,
    db: web::Data<DatabaseConnection>,
    body: web::Json<LoginBody>,
) -> Result<HttpResponse, ServiceError> {
    let fp = extract_client_fingerprint(&req);
    let key = format!("login:{}:{}", body.uuid, fp);

    RATE_LIMITER.check_login_blocked(&key, 5)?;

    let res = auth_service::login_service(
        auth_service::LoginInput {
            uuid: body.uuid.clone(),
            password: body.password.clone(),
        },
        &db,
    )
    .await;

    match res {
        Ok(out) => {
            RATE_LIMITER.reset(&key);
            Ok(HttpResponse::Ok().json(out))
        }
        Err(ServiceError::InvalidCredentials) => {
            let _ = RATE_LIMITER.record_login_failure(&key, 5, Duration::from_secs(60));
            Err(ServiceError::InvalidCredentials)
        }
        Err(e) => Err(e),
    }
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
    req: HttpRequest,
    db: web::Data<DatabaseConnection>,
    auth: Auth,
    body: web::Json<ChangeBody>,
) -> Result<HttpResponse, ServiceError> {
    check_jwt_rate_limit(&auth, &req, "auth_change", 10, 60)?;
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

async fn logout(auth: Auth) -> HttpResponse {
    if let Some(jti) = &auth.claims.jti {
        revoke_token(jti);
    }
    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "logged out"
    }))
}
