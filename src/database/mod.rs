use sea_orm::{Database, DatabaseConnection, DbErr};

/// Connect using DATABASE_URL (loaded from .env by main).
/// There is deliberately NO hardcoded fallback URL: fail fast at startup
/// instead of silently pointing at a developer's local database.
pub async fn establish_connection() -> Result<DatabaseConnection, DbErr> {
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set (see .env.example)");
    Database::connect(url).await
}
