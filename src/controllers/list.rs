use actix_web::{web, Scope};
use crate::controllers::items::post_items_add_controller;

pub fn list_scope() -> Scope {web::scope("/")
    .route("/" , web::post().to(post_items_add_controller))
}