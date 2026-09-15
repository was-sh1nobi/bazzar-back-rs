//! `GET /balance?player=uuid` — per guide §9.2 (no auth, plugin contract).

use actix_web::{HttpRequest, HttpResponse, Scope, web};
use sea_orm::DatabaseConnection;
use serde::Deserialize;

use crate::auth::Auth;
use crate::services::{balance as balance_service, error::ServiceError};
use crate::utils::rate_limit::check_jwt_rate_limit;

pub fn balance_scope() -> Scope {
    web::scope("/balance")
        .route("", web::get().to(get_balance))
        .route("/set", web::post().to(gone_balance))
        .route("/change", web::post().to(gone_balance))
}

#[derive(Deserialize)]
struct BalanceQuery {
    player: Option<String>,
}

async fn get_balance(
    req: HttpRequest,
    auth: Auth,
    query: web::Query<BalanceQuery>,
    db: web::Data<DatabaseConnection>,
) -> Result<HttpResponse, ServiceError> {
    check_jwt_rate_limit(&auth, &req, "balance_read", 120, 60)?;
    let Some(player) = query.player.as_deref() else {
        return Err(ServiceError::MissingParam(
            "player query param is required".to_owned(),
        ));
    };
    if player != auth.player_uuid() {
        return Err(ServiceError::Unauthorized(
            "you can only query your own balance".to_owned(),
        ));
    }
    let out = balance_service::get_balance_service(player, &db).await?;
    Ok(HttpResponse::Ok().json(out))
}

async fn gone_balance() -> HttpResponse {
    // ponytail: static 410 response per guide §10 #12/13
    HttpResponse::Gone().json(serde_json::json!({
        "success": false,
        "message": "Gone — balance mutations are private"
    }))
}
