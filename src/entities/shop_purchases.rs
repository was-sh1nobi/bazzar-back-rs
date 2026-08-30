use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "shop_purchases")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub playeruuid: String,
    #[sea_orm(column_name = "itemId")]
    pub item_id: String,
    pub material: String,
    pub amount: i32,
    #[sea_orm(column_name = "priceEach")]
    pub price_each: f64,
    #[sea_orm(column_name = "totalCost")]
    pub total_cost: f64,
    pub created_at: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::Playeruuid",
        to = "super::users::Column::Playeruuid",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Users,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
