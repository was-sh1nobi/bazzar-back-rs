use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260830_030531_add_users_banned"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        // Node db.ts:111 — users.banned INTEGER NOT NULL DEFAULT 0
        let _ = db
            .execute_unprepared(
                "ALTER TABLE users ADD COLUMN IF NOT EXISTS banned INTEGER NOT NULL DEFAULT 0",
            )
            .await;
        // Node cleanup: drop legacy role column if exists
        let _ = db
            .execute_unprepared("ALTER TABLE users DROP COLUMN IF EXISTS role")
            .await;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let _ = db
            .execute_unprepared("ALTER TABLE users DROP COLUMN IF EXISTS banned")
            .await;
        Ok(())
    }
}
