use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260830_030601_create_shop_items"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ShopItems::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ShopItems::Id).text().not_null().primary_key())
                    .col(ColumnDef::new(ShopItems::Material).text().not_null().unique_key())
                    .col(ColumnDef::new(ShopItems::Category).text().not_null())
                    .col(
                        ColumnDef::new(ShopItems::Buy)
                            .double()
                            .not_null()
                            .check(Expr::cust("buy >= 0")),
                    )
                    .col(
                        ColumnDef::new(ShopItems::Sell)
                            .double()
                            .not_null()
                            .check(Expr::cust("sell >= 0")),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_shop_material")
                    .table(ShopItems::Table)
                    .col(ShopItems::Material)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_shop_category")
                    .table(ShopItems::Table)
                    .col(ShopItems::Category)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ShopItems::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ShopItems {
    Table,
    Id,
    Material,
    Category,
    Buy,
    Sell,
}
