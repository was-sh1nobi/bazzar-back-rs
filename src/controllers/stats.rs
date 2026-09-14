use crate::auth::Auth;
use crate::services::error::ServiceError;
use crate::services::stats::get_stats_service;
use actix_web::{HttpResponse, Scope, web};
use sea_orm::DatabaseConnection;
use serde::Deserialize;

pub fn stats_scope() -> Scope {
    web::scope("/stats").route("", web::get().to(get_stats))
}

#[derive(Deserialize)]
pub struct StatsRequest {
    player: Option<String>,
}
pub async fn get_stats(
    auth: Auth,
    query: web::Query<StatsRequest>,
    db: web::Data<DatabaseConnection>,
) -> Result<HttpResponse, ServiceError> {
    let Some(player) = query.player.as_deref() else {
        return Err(ServiceError::MissingParam(
            "player query param is required".to_string(),
        ));
    };
    if player != auth.player_uuid() {
        return Err(ServiceError::Unauthorized(
            "you can only query your own stats".to_string(),
        ));
    }
    let out = get_stats_service(player, &db).await?;
    Ok(HttpResponse::Ok().json(out))
}
