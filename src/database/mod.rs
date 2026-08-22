use sea_orm::{Database, DatabaseConnection, DbErr};

pub async fn establish_connection() -> Result<DatabaseConnection, DbErr> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://amir:amir1388@localhost:5432/bazzardb".to_string());
    Database::connect(url).await
}

pub mod version {
    use std::sync::atomic::{AtomicU64, Ordering};

    pub static MUTATION_VERSION: AtomicU64 = AtomicU64::new(0);

    pub fn bump_version() {
        MUTATION_VERSION.fetch_add(1, Ordering::SeqCst);
    }

    pub fn get_version() -> u64 {
        MUTATION_VERSION.load(Ordering::SeqCst)
    }
}
