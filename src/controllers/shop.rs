//! HTTP plumbing for /shop routes (catalog, reload, buy, sell).

use actix_web::{HttpRequest, HttpResponse, Scope, web};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};

use crate::auth::Auth;
use crate::services::error::ServiceError;
use crate::services::shop as shop_service;
use crate::utils::rate_limit::check_jwt_rate_limit;

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
    req: HttpRequest,
    auth: Auth,
    db: web::Data<DatabaseConnection>,
) -> Result<HttpResponse, ServiceError> {
    check_jwt_rate_limit(&auth, &req, "shop_reload", 5, 60)?;

    let is_plugin_secret = req
        .headers()
        .get("x-plugin-secret")
        .and_then(|h| h.to_str().ok())
        .and_then(|secret| {
            std::env::var("PLUGIN_SECRET")
                .ok()
                .map(|expected| !expected.is_empty() && expected == secret)
        })
        .unwrap_or(false);

    let is_jwt_admin = auth.role() == Some("admin");

    let is_web_admin = if !is_plugin_secret && !is_jwt_admin {
        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT 1 FROM web_users WHERE username = $1 AND role = 'admin' LIMIT 1",
            [auth.username().into()],
        );
        db.query_one(stmt).await.ok().flatten().is_some()
    } else {
        false
    };

    if !is_plugin_secret && !is_jwt_admin && !is_web_admin {
        return Err(ServiceError::Unauthorized(
            "admin privilege or valid plugin secret required".to_owned(),
        ));
    }

    let reloaded = shop_service::reload_shop_catalog_service(&db).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": format!("reloaded {reloaded} items")
    })))
}

async fn post_buy(
    req: HttpRequest,
    auth: Auth,
    body: web::Json<shop_service::ShopBuyBody>,
    db: web::Data<DatabaseConnection>,
) -> Result<HttpResponse, ServiceError> {
    check_jwt_rate_limit(&auth, &req, "shop_buy", 30, 60)?;
    let out = shop_service::shop_buy_service(auth.player_uuid(), body.into_inner(), &db).await?;
    Ok(HttpResponse::Ok().json(out))
}

async fn post_sell(
    req: HttpRequest,
    auth: Auth,
    body: web::Json<shop_service::ShopSellBody>,
    db: web::Data<DatabaseConnection>,
) -> Result<HttpResponse, ServiceError> {
    check_jwt_rate_limit(&auth, &req, "shop_sell", 20, 60)?;
    let out = shop_service::shop_sell_service(auth.player_uuid(), body.into_inner(), &db).await?;
    Ok(HttpResponse::Ok().json(out))
}
