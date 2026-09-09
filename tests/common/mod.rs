use actix_web::web;
use bazzar_back::controllers::{
    auth::auth_scope, balance::balance_scope, health::health, items::items_scope, list::list_scope,
    stats::stats_scope,
};
use sea_orm::DatabaseConnection;

#[allow(dead_code)]
pub fn configure(config: &mut web::ServiceConfig) {
    config
        .app_data(web::Data::new(DatabaseConnection::default()))
        .service(auth_scope())
        .service(balance_scope())
        .service(stats_scope())
        .service(items_scope())
        .service(list_scope())
        .service(health);
}
