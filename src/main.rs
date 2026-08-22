pub mod controllers;
pub mod database;
pub mod entities;
pub mod services;
pub mod utils;

use actix_web::{App, HttpServer};
use crate::controllers::auth::auth_scope;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(auth_scope())
    })
    .bind(("127.0.0.1" , 8080))?
    .run().await
}