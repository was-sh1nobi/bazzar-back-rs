use sea_orm_migration::{prelude::*, schema::*};
use sea_orm_migration::sea_orm::sea_query::extension::postgres::Type;
pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260822_172834_create_items_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type(
                Type::create()
                    .as_enum(ItemStatus::Enum)
                    .values([
                        ItemStatus::Available,
                        ItemStatus::Sold,
                        ItemStatus::Reserved,
                        ItemStatus::Removed,
                    ])
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Items::Table)
                    .if_not_exists()
                    .col(pk_auto(Items::Id))
                    .col(text(Items::Seller).not_null())
                    .col(text(Items::Material))
                    .col(integer(Items::Amount))
                    .col(text(Items::Price))
                    .col(text(Items::Source))
                    .col(text(Items::Origin))
                    .col(
                        enumeration(Items::Status, ItemStatus::Enum, [
                            ItemStatus::Available,
                            ItemStatus::Sold,
                            ItemStatus::Reserved,
                            ItemStatus::Removed,
                        ])
                            .not_null()
                            .default(ItemStatus::Available.to_string()),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Items::Table).to_owned())
            .await?;
        manager
            .drop_type(Type::drop().name(ItemStatus::Enum).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Items {
    Table,
    Id,
    Seller,
    Material,
    Amount,
    Price,
    Origin,
    Source,
    Status,
}

#[derive(DeriveIden)]
enum ItemStatus {
    #[sea_orm(iden = "item_status")]
    Enum,
    #[sea_orm(iden = "AVAILABLE")]
    Available,
    #[sea_orm(iden = "SOLD")]
    Sold,
    #[sea_orm(iden = "RESERVED")]
    Reserved,
    #[sea_orm(iden = "REMOVED")]
    Removed,
}