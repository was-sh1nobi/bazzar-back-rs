pub use sea_orm_migration::prelude::*;

mod m20260822_172834_create_items_table;
mod m20260822_174939_create_history_table;


pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260822_172834_create_items_table::Migration),
            Box::new(m20260822_174939_create_history_table::Migration),
        ]
    }
}
