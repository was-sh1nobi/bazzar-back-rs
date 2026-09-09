//! `GET /items/poll` long-polling service (spec §10 #3, Node `routes/items.ts:40`).
//!
//! Mirrors the Node contract exactly:
//! - `since`  : int, default 0 — the mutation version the caller last saw
//! - `timeout`: seconds, clamped 1..60, default 25 — max time to hold the request
//!
//! Busy-waits (500ms wake, same as Node) until the global mutation version is
//! greater than `since` or the deadline passes, then re-runs the SAME query as
//! `GET /items` (`items_main::get_items_main_service`) so the poll response is
//! byte-identical to the main feed envelope. No duplicated SQL — the poll is a
//! thin wait-layer over `items_main`.

use std::time::{Duration, Instant};

use sea_orm::DatabaseConnection;

use crate::services::error::ServiceError;
use crate::services::items::items_main::{ItemsMainOutput, get_items_main_service};
use crate::services::items::version::get_version;

/// 500ms wake interval — same cadence as Node `sleep(500)` (`routes/items.ts:40`).
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Long-poll: wait until the mutation version moves past `since`, then serve the
/// standard items envelope. Always returns the envelope (even on timeout, like
/// the Node implementation) — the client compares `version` to detect changes.
pub async fn get_items_poll_service(
    page: Option<i64>,
    page_size: Option<i64>,
    uuid: Option<&str>,
    since: Option<i64>,
    timeout: Option<u64>,
    db: &DatabaseConnection,
) -> Result<ItemsMainOutput, ServiceError> {
    let since = since.unwrap_or(0).max(0);
    // Node: `Math.min(Math.max(timeout, 1), 60)`, default 25
    let timeout = timeout.unwrap_or(25).clamp(1, 60);
    let deadline = Instant::now() + Duration::from_secs(timeout);

    // Busy-wait until something changed, or the client's deadline is hit.
    while get_version() <= since {
        if Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }

    // Reuse the main items service — identical envelope, provenance filter,
    // pagination and own-listing exclusion. Zero extra SQL here.
    get_items_main_service(page, page_size, uuid, db).await
}
