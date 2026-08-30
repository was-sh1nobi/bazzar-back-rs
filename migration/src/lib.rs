pub use sea_orm_migration::prelude::*;

mod m20260822_172834_create_items_table;
mod m20260822_174939_create_history_table;
mod m20260823_000001_create_users_table;
mod m20260823_000002_patch_spec_compliance;
mod m20260823_204632_create_catalog_table;
mod m20260830_030531_add_users_banned;
mod m20260830_030601_create_web_users;
mod m20260830_030601_create_shop_items;
mod m20260830_030601_create_shop_purchases;
mod m20260901_add_items_playeruuid;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260822_172834_create_items_table::Migration),
            Box::new(m20260822_174939_create_history_table::Migration),
            Box::new(m20260823_000001_create_users_table::Migration),
            Box::new(m20260823_000002_patch_spec_compliance::Migration),
            Box::new(m20260823_204632_create_catalog_table::Migration),
            Box::new(m20260830_030531_add_users_banned::Migration),
            Box::new(m20260830_030601_create_web_users::Migration),
            Box::new(m20260830_030601_create_shop_items::Migration),
            Box::new(m20260830_030601_create_shop_purchases::Migration),
            Box::new(m20260901_add_items_playeruuid::Migration),
        ]
    }
}
