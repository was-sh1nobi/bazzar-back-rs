use actix_web::{Scope, web, HttpResponse};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use crate::auth::Auth;
use crate::services::error::ServiceError;
use crate::services::items::items_add::{create_listing, ItemsAddBody};
use crate::services::items::items_buy::items_buy_service;
use crate::services::items::items_main::get_items_main_service;
use crate::services::items::items_owned::{
    get_items_active_service, get_items_bought_service, get_items_history_service,
};
use crate::services::items::items_poll::get_items_poll_service;

pub fn items_scope() -> Scope { web::scope("/items")
    .route("/", web::get().to(get_items_controller))
    .route("/poll", web::get().to(get_items_poll_controller))
    .route("/active", web::get().to(get_items_active_controller))
    .route("/bought", web::get().to(get_items_bought_controller))
    .route("/history", web::get().to(get_items_history_controller))
    .route("/add", web::post().to(post_items_add_controller))
    .route("/buy", web::post().to(post_items_buy_controller))
}



#[derive(Deserialize)]
pub struct ItemsMainInput {
    page : Option<i64>,
    #[serde(rename = "pageSize")]
    page_size: Option<i64>,
    player : Option<String>,
}

/// `GET /items/poll?page&pageSize&player&since&timeout` — long-poll variant of
/// `GET /items` (spec §10 #3). `since` defaults to 0, `timeout` is seconds
/// clamped 1..60 (default 25, matching the plugin's 30s client timeout headroom).
#[derive(Deserialize)]
pub struct ItemsPollInput {
    page : Option<i64>,
    #[serde(rename = "pageSize")]
    page_size: Option<i64>,
    player : Option<String>,
    since : Option<i64>,
    timeout : Option<u64>,
}

pub async fn get_items_controller(query : web::Query<ItemsMainInput>, db : web::Data<DatabaseConnection>) -> Result<HttpResponse, ServiceError> {
    let uuid = query.player.as_deref();
    let out = get_items_main_service(query.page , query.page_size , uuid , &db ).await?;
    Ok(HttpResponse::Ok().json(out))
}

pub async fn get_items_poll_controller(query : web::Query<ItemsPollInput>, db : web::Data<DatabaseConnection>) -> Result<HttpResponse, ServiceError> {
    let uuid = query.player.as_deref();
    let out = get_items_poll_service(
        query.page,
        query.page_size,
        uuid,
        query.since,
        query.timeout,
        &db,
    ).await?;
    Ok(HttpResponse::Ok().json(out))
}

/// Shared pagination-only query for the JWT-scoped ownership feeds.
/// Deliberately NO `player` field — identity comes exclusively from
/// `auth.claims.sub`, so a caller can never read another player's data.
#[derive(Deserialize)]
pub struct ItemsOwnedInput {
    page : Option<i64>,
    #[serde(rename = "pageSize")]
    page_size: Option<i64>,
}

/// `GET /items/active` — caller's own AVAILABLE listings (Bearer JWT required).
pub async fn get_items_active_controller(auth : Auth, query : web::Query<ItemsOwnedInput>, db : web::Data<DatabaseConnection>) -> Result<HttpResponse, ServiceError> {
    let out = get_items_active_service(query.page, query.page_size, auth.player_uuid(), &db).await?;
    Ok(HttpResponse::Ok().json(out))
}

/// `GET /items/bought` — history rows the caller bought (Bearer JWT required).
pub async fn get_items_bought_controller(auth : Auth, query : web::Query<ItemsOwnedInput>, db : web::Data<DatabaseConnection>) -> Result<HttpResponse, ServiceError> {
    let out = get_items_bought_service(query.page, query.page_size, auth.player_uuid(), &db).await?;
    Ok(HttpResponse::Ok().json(out))
}

/// `GET /items/history` — everything the caller sold or bought (Bearer JWT required).
pub async fn get_items_history_controller(auth : Auth, query : web::Query<ItemsOwnedInput>, db : web::Data<DatabaseConnection>) -> Result<HttpResponse, ServiceError> {
    let out = get_items_history_service(query.page, query.page_size, auth.player_uuid(), &db).await?;
    Ok(HttpResponse::Ok().json(out))
}

/// `POST /items/add` — create a listing. Seller identity comes ONLY from the
/// JWT (`auth.claims.sub`); the body's material/amount/price/source/origin are
/// validated server-side (guide §10.1/§14). Response matches the Node shape:
/// `200 {success:true, message:"item created with id : <id>"}`.
pub async fn post_items_add_controller(auth : Auth, body : web::Json<ItemsAddBody>, db : web::Data<DatabaseConnection>) -> Result<HttpResponse, ServiceError> {
    let id = create_listing(body.into_inner(), &db, auth.player_uuid()).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": format!("item created with id : {id}")
    })))
}

/// `POST /items/buy` — buy a listing (guide §12.1). Buyer identity comes ONLY
/// from the JWT; the body carries just `{ "wantedId": <id> }`. Fully
/// transactional: PENDING claim → atomic deduct → SOLD → history → seller
/// credit, COMMIT, or ROLLBACK on any failure.
pub async fn post_items_buy_controller(auth : Auth, body : web::Json<ItemsBuyBody>, db : web::Data<DatabaseConnection>) -> Result<HttpResponse, ServiceError> {
    let out = items_buy_service(auth.player_uuid(), body.wanted_id, &db).await?;
    Ok(HttpResponse::Ok().json(out))
}

/// Node body field is `wantedId` — `buyerId` is NOT accepted (JWT wins).
#[derive(Deserialize)]
pub struct ItemsBuyBody {
    #[serde(rename = "wantedId")]
    wanted_id: i64,
}