use crate::controllers::items::post_items_add_controller;
use actix_web::{Scope, web};

pub fn list_scope() -> Scope {
    web::scope("/").route("", web::post().to(post_items_add_controller))
}
