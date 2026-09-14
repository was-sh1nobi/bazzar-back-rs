use actix_web::web;
use bazzar_back::controllers::health::health;
use bazzar_back::controllers::items::post_items_add_controller;
use bazzar_back::controllers::list::{list_scope, root_welcome};
use bazzar_back::controllers::shop::shop_scope;
use bazzar_back::controllers::{
    auth::auth_scope, balance::balance_scope, items::items_scope, stats::stats_scope,
};
use sea_orm::DatabaseConnection;

#[allow(dead_code)]
pub fn configure(config: &mut web::ServiceConfig) {
    config
        .app_data(web::Data::new(DatabaseConnection::default()))
        .service(health)
        .route("/", web::get().to(root_welcome))
        .route("/", web::post().to(post_items_add_controller))
        .service(auth_scope())
        .service(balance_scope())
        .service(stats_scope())
        .service(items_scope())
        .service(list_scope())
        .service(shop_scope());
}
