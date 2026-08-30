//! `GET /balance?player=uuid` — per guide §9.2 (no auth, plugin contract).

use actix_web::{HttpResponse, Scope, web};
use sea_orm::DatabaseConnection;
use serde::Deserialize;

use crate::services::{balance as balance_service, error::ServiceError};

pub fn balance_scope() -> Scope {
    web::scope("/balance").route("", web::get().to(get_balance))
}

#[derive(Deserialize)]
struct BalanceQuery {
    player: Option<String>,
}

async fn get_balance(
    query: web::Query<BalanceQuery>,
    db: web::Data<DatabaseConnection>,
) -> Result<HttpResponse, ServiceError> {
    let Some(player) = query.player.as_deref() else {
        return Err(ServiceError::MissingParam(
            "player query param is required".to_owned(),
        ));
    };
    let out = balance_service::get_balance_service(player, &db).await?;
    Ok(HttpResponse::Ok().json(out))
}
