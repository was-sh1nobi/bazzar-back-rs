//! HTTP plumbing for /shop routes (catalog, reload, buy, sell).

use actix_web::{HttpResponse, Scope, web};
use sea_orm::DatabaseConnection;

use crate::auth::Auth;
use crate::services::error::ServiceError;
use crate::services::shop as shop_service;

pub fn shop_scope() -> Scope {
    web::scope("/shop")
        .route("/catalog", web::get().to(get_catalog))
        .route("/catalog/reload", web::post().to(post_reload))
        .route("/buy", web::post().to(post_buy))
        .route("/sell", web::post().to(post_sell))
}

async fn get_catalog(db: web::Data<DatabaseConnection>) -> Result<HttpResponse, ServiceError> {
    let out = shop_service::get_shop_catalog_service(&db).await?;
    Ok(HttpResponse::Ok().json(out))
}

async fn post_reload(
    _auth: Auth,
    db: web::Data<DatabaseConnection>,
) -> Result<HttpResponse, ServiceError> {
    let reloaded = shop_service::reload_shop_catalog_service(&db).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": format!("reloaded {reloaded} items")
    })))
}

async fn post_buy(
    auth: Auth,
    body: web::Json<shop_service::ShopBuyBody>,
    db: web::Data<DatabaseConnection>,
) -> Result<HttpResponse, ServiceError> {
    let out = shop_service::shop_buy_service(auth.player_uuid(), body.into_inner(), &db).await?;
    Ok(HttpResponse::Ok().json(out))
}

async fn post_sell(
    auth: Auth,
    body: web::Json<shop_service::ShopSellBody>,
    db: web::Data<DatabaseConnection>,
) -> Result<HttpResponse, ServiceError> {
    let out = shop_service::shop_sell_service(auth.player_uuid(), body.into_inner(), &db).await?;
    Ok(HttpResponse::Ok().json(out))
}
