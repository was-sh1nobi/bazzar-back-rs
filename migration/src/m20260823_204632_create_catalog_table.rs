use sea_orm_migration::{prelude::*, schema::*};
use sea_orm_migration::sea_orm::sea_query::extension::postgres::Type;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260823_204632_create_catalog_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Create enum type catalog_status
        manager
            .create_type(
                Type::create()
                    .as_enum(CatalogStatus::Enum)
                    .values([
                        CatalogStatus::Available,
                        CatalogStatus::Unavailable,
                        CatalogStatus::Removed,
                        CatalogStatus::ComingSoon,
                    ])
                    .to_owned(),
            )
            .await?;

        // 2. Create catalog table
        manager
            .create_table(
                Table::create()
                    .table(Catalog::Table)
                    .if_not_exists()
                    .col(pk_auto(Catalog::Id))
                    .col(text(Catalog::Material))
                    .col(text_null(Catalog::Category))
                    .col(
                        ColumnDef::new(Catalog::BuyPrice)
                            .double()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Catalog::SellPrice)
                            .double()
                            .not_null(),
                    )
                    .col(
                        enumeration(
                            Catalog::Status,
                            CatalogStatus::Enum,
                            [
                                CatalogStatus::Available,
                                CatalogStatus::Unavailable,
                                CatalogStatus::Removed,
                                CatalogStatus::ComingSoon,
                            ],
                        )
                        .not_null()
                        .default(CatalogStatus::Available.to_string()),
                    )
                    .to_owned(),
            )
            .await?;

        // 3. Indexes
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_catalog_status")
                    .table(Catalog::Table)
                    .col(Catalog::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_catalog_material")
                    .table(Catalog::Table)
                    .col(Catalog::Material)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Catalog::Table).to_owned())
            .await?;
        manager
            .drop_type(Type::drop().name(CatalogStatus::Enum).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Catalog {
    Table,
    Id,
    Material,
    Category,
    BuyPrice,
    SellPrice,
    Status,
}
#[derive(DeriveIden)]
enum CatalogStatus {
    #[sea_orm(iden = "catalog_status")]
    Enum,
    #[sea_orm(iden = "AVAILABLE")]
    Available,
    #[sea_orm(iden = "UNAVAILABLE")]
    Unavailable,
    #[sea_orm(iden = "REMOVED")]
    Removed,
    #[sea_orm(iden = "COMING SOON")]
    ComingSoon,
}
