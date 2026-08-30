use actix_web::{web, HttpResponse, Scope};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use crate::auth::Auth;
use crate::services::error::ServiceError;
use crate::services::stats::get_stats_service;

pub fn stats_scope() -> Scope { web::scope("/stats").route("", web::get().to(get_stats)) }

#[derive(Deserialize)]
pub struct StatsRequest {
    player : Option<String>,
}
pub async fn get_stats(
    _auth: Auth,
    query: web::Query<StatsRequest>,
    db: web::Data<DatabaseConnection>,
) -> Result<HttpResponse, ServiceError> {
    let Some(player) = query.player.as_deref() else {
        return Err(ServiceError::MissingParam(
            "player query param is required".to_string(),
        ));
    };
    // Optional hardening: users may only query their own stats unless privileged.
    // For now allow any `player` if you have a valid JWT, but you can enforce:
    // if player != auth.player_uuid() { return Err(ServiceError::Unauthorized(...)) }
    let out = get_stats_service(player, &db).await?;
    Ok(HttpResponse::Ok().json(out))
}