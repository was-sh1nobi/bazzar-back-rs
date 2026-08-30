use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260830_030601_create_shop_purchases"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ShopPurchases::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ShopPurchases::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ShopPurchases::PlayerUuid).text().not_null())
                    .col(ColumnDef::new(ShopPurchases::ItemId).text().not_null())
                    .col(ColumnDef::new(ShopPurchases::Material).text().not_null())
                    .col(ColumnDef::new(ShopPurchases::Amount).integer().not_null())
                    .col(ColumnDef::new(ShopPurchases::PriceEach).double().not_null())
                    .col(ColumnDef::new(ShopPurchases::TotalCost).double().not_null())
                    .col(
                        ColumnDef::new(ShopPurchases::CreatedAt)
                            .big_integer()
                            .null()
                            .default(Expr::cust("extract(epoch from now())::bigint")),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_shop_purchases_playeruuid")
                            .from(ShopPurchases::Table, ShopPurchases::PlayerUuid)
                            .to(Users::Table, Users::PlayerUuid)
                            .on_delete(ForeignKeyAction::NoAction)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_shop_purchases_player")
                    .table(ShopPurchases::Table)
                    .col(ShopPurchases::PlayerUuid)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ShopPurchases::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    #[sea_orm(iden = "playeruuid")]
    PlayerUuid,
}

#[derive(DeriveIden)]
enum ShopPurchases {
    Table,
    Id,
    PlayerUuid,
    ItemId,
    Material,
    Amount,
    PriceEach,
    TotalCost,
    CreatedAt,
}
