use actix_web::{App, HttpServer, middleware::Logger, web};
use bazzar_back::controllers::{auth::auth_scope, balance::balance_scope, items::items_scope, stats::stats_scope};
use bazzar_back::controllers::list::list_scope;
use bazzar_back::database;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // .env must be loaded BEFORE anything reads env vars
    dotenvy::dotenv().ok();

    // Structured logging; RUST_LOG filter, defaults to info
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,actix_web=info".into()),
        )
        .init();

    // Fail fast if the database is unreachable
    let db = database::establish_connection()
        .await
        .expect("failed to connect to database — check DATABASE_URL");

    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    tracing::info!("bazzar-back listening on http://{host}:{port}");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(db.clone()))
            .wrap(Logger::default())
            .service(auth_scope())
            .service(balance_scope())
            .service(stats_scope())
            .service(items_scope())
            .service(list_scope())
    })
    .bind((host.as_str(), port))?
    .run()
    .await
}
