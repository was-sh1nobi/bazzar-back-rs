use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "history")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub seller: Option<String>,
    pub buyer: Option<String>,
    pub material: Option<String>,
    pub price: Option<String>,
    pub amount: Option<i32>,
    pub source: Option<String>,
    pub origin: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::Seller",
        to = "super::users::Column::Username",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    SellerUser,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::Buyer",
        to = "super::users::Column::Playeruuid",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    BuyerUser,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SellerUser.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
