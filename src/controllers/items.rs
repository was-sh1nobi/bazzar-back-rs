use actix_web::{Scope, web, HttpResponse};
use sea_orm::DatabaseConnection;
use sea_orm::sqlx::query;
use serde::Deserialize;
use crate::services::error::ServiceError;
use crate::services::items::items_main::get_items_main_service;

pub fn items_scope() -> Scope { web::scope("/items")
    .route("/", web::get().to(get_items_controller))
    


}



#[derive(Deserialize)]
pub struct ItemsMainInput {
    page : Option<i64>,
    page_size: Option<i64>,
    player : Option<String>,
}

pub async fn get_items_controller(query : web::Query<ItemsMainInput>, db : web::Data<DatabaseConnection>) -> Result<HttpResponse, ServiceError> {
    let uuid = query.player.as_deref();
    let out = get_items_main_service(query.page , query.page_size , uuid , &db ).await?;
    Ok(HttpResponse::Found().json(out))
}