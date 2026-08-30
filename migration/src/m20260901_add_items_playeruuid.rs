use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260901_add_items_playeruuid"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Add seller_playeruuid column to items for direct UUID queries (Node stores seller as name only)
        let _ = db
            .execute_unprepared(
                "ALTER TABLE items ADD COLUMN IF NOT EXISTS seller_playeruuid TEXT",
            )
            .await;

        // Backfill from users where possible (best-effort, no error if no match)
        let _ = db
            .execute_unprepared(
                "UPDATE items SET seller_playeruuid = u.playeruuid FROM users u WHERE items.seller = u.username AND items.seller_playeruuid IS NULL",
            )
            .await;

        let _ = db
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_items_seller_playeruuid ON items(seller_playeruuid)",
            )
            .await;

        // Also add seller_playeruuid to history for parity (history.seller is name, buyer is uuid)
        let _ = db
            .execute_unprepared(
                "ALTER TABLE history ADD COLUMN IF NOT EXISTS seller_playeruuid TEXT",
            )
            .await;
        let _ = db
            .execute_unprepared(
                "UPDATE history SET seller_playeruuid = u.playeruuid FROM users u WHERE history.seller = u.username AND history.seller_playeruuid IS NULL",
            )
            .await;
        let _ = db
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_history_seller_playeruuid ON history(seller_playeruuid)",
            )
            .await;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let _ = db
            .execute_unprepared("DROP INDEX IF EXISTS idx_items_seller_playeruuid")
            .await;
        let _ = db
            .execute_unprepared("DROP INDEX IF EXISTS idx_history_seller_playeruuid")
            .await;
        let _ = db
            .execute_unprepared("ALTER TABLE items DROP COLUMN IF EXISTS seller_playeruuid")
            .await;
        let _ = db
            .execute_unprepared("ALTER TABLE history DROP COLUMN IF EXISTS seller_playeruuid")
            .await;
        Ok(())
    }
}
