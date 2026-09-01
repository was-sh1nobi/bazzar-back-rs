//! `POST /items/buy` — buy an AH listing (guide §12.1, Node `routes/items.ts:114`).
//!
//! SECURITY: buyer identity comes ONLY from the JWT (`auth.claims.sub`).
//!
//! ORDERING: the FIRST statement of the transaction is the PENDING claim
//! (`UPDATE ... WHERE status='AVAILABLE' RETURNING ...`) — the linearization
//! point that stops two buyers racing for the same listing. Every other step
//! (own-listing check, atomic balance deduction, SOLD flip, history insert,
//! seller credit) runs in the SAME transaction; ANY failure rolls everything
//! back, so a failed buy never leaves a listing PENDING or money deducted.

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait,
};
use serde::Serialize;

use crate::services::error::{internal, ServiceError};
use crate::services::items::version::bump_version;
use crate::utils::validation::validate_uuid;

#[derive(Serialize)]
pub struct ItemsBuyOutput {
    success: bool,
    /// Post-deduction buyer balance (users.balance is double precision → f64).
    balance: f64,
    /// Node shape: "item added with id : <historyId>"
    message: String,
    /// Global mutation version AFTER this purchase (wakes /items/poll).
    bump: i64,
}

pub async fn items_buy_service(
    uuid: &str,
    item_to_buy: i64,
    db: &DatabaseConnection,
) -> Result<ItemsBuyOutput, ServiceError> {
    if !validate_uuid(uuid) {
        return Err(ServiceError::MalformedUuid);
    }
    // Non-positive ids can never exist — reject before touching the DB.
    if item_to_buy <= 0 {
        return Err(ServiceError::ItemNotFound);
    }

    let tx = db.begin().await.map_err(|e| internal("items buy: begin tx", e))?;

    // All transactional work; on Err the outer match rolls the tx back.
    let work = async {
        // ---- 1. FIRST query: claim the listing (linearization point) --------
        let claim = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"UPDATE items SET status = 'PENDING'
               WHERE id = $1 AND status = 'AVAILABLE'
               RETURNING seller, seller_playeruuid, material, amount, price"#,
            [item_to_buy.into()],
        );
        let claimed = tx
            .query_one(claim)
            .await
            .map_err(|e| internal("items buy: claim failed", e))?;

        let Some(claimed) = claimed else {
            // 0 rows: missing listing vs not available -> distinguish (§12.1)
            let probe = Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"SELECT status::text AS status FROM items WHERE id = $1"#,
                [item_to_buy.into()],
            );
            let status: Option<String> = tx
                .query_one(probe)
                .await
                .map_err(|e| internal("items buy: probe failed", e))?
                .and_then(|row| row.try_get("", "status").ok());
            return match status.as_deref() {
                None => Err(ServiceError::ItemNotFound),
                Some("PENDING") => Err(ServiceError::InvalidInput("processing".to_owned())),
                Some(_) => Err(ServiceError::InvalidInput("sold out".to_owned())),
            };
        };

        // ---- 2. Own-listing check (uuid compare — exact, no name join) ------
        let seller: String = claimed.try_get("", "seller").unwrap_or_default();
        let seller_uuid: Option<String> = claimed.try_get("", "seller_playeruuid").unwrap_or(None);
        let amount: i32 = claimed
            .try_get::<Option<i32>>("", "amount")
            .unwrap_or(None)
            .unwrap_or(0);
        let price_raw: Option<String> = claimed.try_get("", "price").unwrap_or(None);

        if seller_uuid.as_deref() == Some(uuid) || seller == uuid {
            return Err(ServiceError::InvalidInput(
                "you cannot buy your own listing".to_owned(),
            ));
        }

        // ---- 3. cost from the DB price — client price is never trusted -----
        let cost = price_raw
            .as_deref()
            .and_then(|p| p.parse::<f64>().ok())
            .map(|p| p * amount as f64)
            .filter(|c| c.is_finite() && *c > 0.0);
        let Some(cost) = cost else {
            return Err(ServiceError::InvalidInput("invalid item price".to_owned()));
        };

        // ---- 4. ATOMIC deduction: 0 rows => insufficient funds -------------
        let deduct = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"UPDATE users SET balance = balance - $2
               WHERE playeruuid = $1 AND balance - $2 >= 0
               RETURNING balance AS balance"#,
            [uuid.into(), cost.into()],
        );
        let new_balance: f64 = match tx
            .query_one(deduct)
            .await
            .map_err(|e| internal("items buy: deduct failed", e))?
        {
            Some(row) => row
                .try_get("", "balance")
                .map_err(|e| internal("items buy: decode balance", e))?,
            None => {
                // Build the Node-style message with the (unchanged) balance
                let bal_stmt = Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    r#"SELECT balance FROM users WHERE playeruuid = $1"#,
                    [uuid.into()],
                );
                let cur = tx
                    .query_one(bal_stmt)
                    .await
                    .map_err(|e| internal("items buy: balance read failed", e))?
                    .and_then(|r| r.try_get::<f64>("", "balance").ok())
                    .unwrap_or(0.0);
                return Err(ServiceError::InvalidInput(format!(
                    "insufficient funds (balance {cur}, cost {cost})"
                )));
            }
        };

        // ---- 5. flip to SOLD ------------------------------------------------
        let sold = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"UPDATE items SET status = 'SOLD' WHERE id = $1"#,
            [item_to_buy.into()],
        );
        tx.execute(sold)
            .await
            .map_err(|e| internal("items buy: sold flip failed", e))?;

        // ---- 6. history row (buyer = UUID, seller = name — guide §5.3) ------
        let history = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"INSERT INTO history (seller, seller_playeruuid, buyer, material, price, amount, source, origin)
               SELECT seller, seller_playeruuid, $1, material, price, amount, source, origin
               FROM items WHERE id = $2
               RETURNING id"#,
            [uuid.into(), item_to_buy.into()],
        );
        let history_id: i32 = tx
            .query_one(history)
            .await
            .map_err(|e| internal("items buy: history insert failed", e))?
            .and_then(|r| r.try_get("", "id").ok())
            .ok_or_else(|| internal("items buy: history id missing", "no RETURNING row"))?;

        // ---- 7. seller credit (best-effort, like Node creditIfRegistered;
        //         an unregistered seller simply matches 0 rows) ---------------
        if let Some(seller_id) = seller_uuid {
            let credit = Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"UPDATE users SET balance = balance + $1 WHERE playeruuid = $2"#,
                [cost.into(), seller_id.into()],
            );
            tx.execute(credit)
                .await
                .map_err(|e| internal("items buy: seller credit failed", e))?;
        }

        Ok((history_id as i64, new_balance))
    };

    // ---- COMMIT on success / ROLLBACK on ANY failure ------------------------
    match work.await {
        Ok((history_id, balance)) => {
            tx.commit()
                .await
                .map_err(|e| internal("items buy: commit failed", e))?;
            let bump = bump_version();
            Ok(ItemsBuyOutput {
                success: true,
                balance,
                message: format!("item added with id : {history_id}"),
                bump,
            })
        }
        Err(e) => {
            // Reverts the PENDING claim, the deduction, SOLD flip, history and
            // credit — the listing stays AVAILABLE and money untouched.
            let _ = tx.rollback().await;
            Err(e)
        }
    }
}
