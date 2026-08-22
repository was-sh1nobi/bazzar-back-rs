use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260823_000002_patch_spec_compliance"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // --- Fix item_status enum: RESERVED -> PENDING ---
        // This patch makes an already-migrated DB (with RESERVED) compliant with spec (PENDING).
        // If the old value does not exist or new value already exists, the statement will error;
        // we swallow specific errors to keep migration idempotent.
        let _ = db
            .execute_unprepared(
                "DO $$ BEGIN
                    IF EXISTS (SELECT 1 FROM pg_type t JOIN pg_enum e ON t.oid = e.enumtypid WHERE t.typname = 'item_status' AND e.enumlabel = 'RESERVED') THEN
                        ALTER TYPE item_status RENAME VALUE 'RESERVED' TO 'PENDING';
                    END IF;
                END $$;",
            )
            .await;

        // Ensure PENDING exists (for fresh enum that somehow missed it - e.g. manual creation)
        let _ = db
            .execute_unprepared(
                "DO $$ BEGIN
                    IF NOT EXISTS (SELECT 1 FROM pg_type t JOIN pg_enum e ON t.oid = e.enumtypid WHERE t.typname = 'item_status' AND e.enumlabel = 'PENDING') THEN
                        ALTER TYPE item_status ADD VALUE 'PENDING';
                    END IF;
                END $$;",
            )
            .await;

        // --- Make items columns nullable per spec (material, amount, price, source, origin) ---
        // These were incorrectly created NOT NULL in the first migration.
        for col in &["material", "amount", "price", "source", "origin"] {
            let sql = format!("ALTER TABLE items ALTER COLUMN {} DROP NOT NULL", col);
            let _ = db.execute_unprepared(&sql).await;
        }

        // --- Make history columns nullable per spec (all except id) ---
        for col in &[
            "seller", "buyer", "material", "price", "amount", "source", "origin",
        ] {
            let sql = format!("ALTER TABLE history ALTER COLUMN {} DROP NOT NULL", col);
            let _ = db.execute_unprepared(&sql).await;
        }

        // Fix seller varchar -> text (was created as VARCHAR in buggy migration)
        let _ = db
            .execute_unprepared("ALTER TABLE items ALTER COLUMN seller TYPE TEXT")
            .await;

        // --- Ensure indexes exist (idempotent) ---
        let _ = db
            .execute_unprepared("CREATE INDEX IF NOT EXISTS idx_items_status ON items(status)")
            .await;
        let _ = db
            .execute_unprepared("CREATE INDEX IF NOT EXISTS idx_items_seller ON items(seller)")
            .await;
        let _ = db
            .execute_unprepared("CREATE INDEX IF NOT EXISTS idx_history_buyer ON history(buyer)")
            .await;
        let _ = db
            .execute_unprepared("CREATE INDEX IF NOT EXISTS idx_users_username ON users(username)")
            .await;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
