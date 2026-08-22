# Bazzar Backend — Migration Guide (Language-Agnostic Spec)

> Single source of truth for porting the backend to another language (Go / Java / Rust / Python / Node alternative). Everything primary: endpoints, logic, DB, auth, quirks.
> Generated 2026-08-22 — covers codebase at `src/` + `api.md` + `.env.example` + tests.

---

## 1. Overview

**Bazzar** is a Minecraft Auction House (AH) + server-side economy backend consumed by a Paper/Spigot plugin over HTTP (off main thread, 30s client timeout). Money has no client state — all balances live server-side. Listings are `ItemStack`s held in hand; the plugin removes from hand before calling the backend and refunds on failure.

```
Minecraft Plugin (Java)  --HTTP JSON-->  Bazzar Backend (Fastify)
                                          |-- SQLite (bazzar.db)
                                          |-- src/data/items.json (shop catalog)
```

---

## 2. Stack to Replicate

| Concern | Current | Target requirement |
|---|---|---|
| Runtime | **Bun** (`bun:sqlite`, `Bun.password`, `bun test`) | Any runtime; replace with native driver + hashing |
| Language | TypeScript (no `tsconfig`) | Target language |
| HTTP | **Fastify v5** (`src/index.ts:10`) | Any framework (Gin, Spring, Axum, FastAPI…) |
| DB | **SQLite WAL** file `./bazzar.db`, `PRAGMA foreign_keys=ON` (`src/db.ts:7-8`) | Keep SQLite or migrate to Postgres; preserve schema+semantics |
| ORM | None — raw SQL via fake `pool` shim (`src/db.ts:92-98`) that mimics `pg` (`query`→`{rows}`, `connect()`→`BEGIN/COMMIT/ROLLBACK`) with `$1,$2` placeholders → rewrite to `?` internally | Use real driver/ORM; **keep `$1` API or translate** |
| Password hashing | `Bun.password.hash/verify` (Argon2id default) (`src/routes/auth.ts:46,62`) | `argon2id` or `bcrypt` — must stay compatible if you keep existing hashes; otherwise force password reset |
| Tests | `bun test` + Fastify `inject()` (`test/api.test.ts`) | Port tests to target framework's test client |
| Logging | `Fastify({logger:true})` + `console.*` | Structured logger |
| Deploy | systemd `deploy/bazzar.service`, `deploy/remote.sh`, GitHub Actions `ci.yml` (`bun test` + SCP deploy) | Adapt |

**Scripts:** `dev` = `bun run --watch src/index.ts`, `test` = `bun test` (`package.json:2-5`).

---

## 3. Project Structure (what each file does)

```
src/index.ts                    App bootstrap, registers route plugins, GET / + GET /stats
src/db.ts                       SQLite open + DDL + migration + pool shim + global version counter
src/routes/items.ts             GET /items, /items/poll, /items/active, /items/bought, /items/history
                                POST /items/add, /items/buy, /items/retrieve
src/routes/auction.ts           POST /list  (modern listing create)
src/routes/balance.ts           GET /balance, POST /balance/set, POST /balance/change
src/routes/auth.ts              GET /auth/status, POST /auth/register|login|change
src/routes/shop.ts              GET /shop/catalog, POST /shop/catalog/reload
src/services/items/items.ts     getItems, getBySeller, getByBuyer, getHistory, getStats + envelope logic
src/services/balance/balance.ts registerIfAbsentByUuid, getBalance, setBalance, changeBalance, creditIfRegistered
src/services/auth/auth.ts       getUserByUuid/Name, ensureUser, setPasswordHash, isRegistered
src/utils/provenance.ts         normalizeSource/Origin, isBlocked — anti-dupe gate
src/utils/registerIfAbsent.ts   INSERT ... ON CONFLICT DO UPDATE username (legacy helper)
src/types/items/items.ts        ItemsAddBody (legacy string fields)
src/data/items.json             Static shop catalog (~300 entries, ~36KB)
test/api.test.ts                Canonical integration tests (use as contract tests)
deploy/bazzar.service           systemd unit
deploy/remote.sh                Remote deploy script
.github/workflows/ci.yml        CI: bun install/test; deploy on push master
api.md                          Authoritative API contract (keep in sync)
```

---

## 4. Configuration & Environment

| Var | Default | Where used | Notes |
|---|---|---|---|
| `DATABASE_URL` | `./bazzar.db` | `src/db.ts:3` | SQLite file path. Target can use `DATABASE_URL` as DSN. |
| `PORT` | `3000` (`.env`/`api.md` use `8080`) | `src/index.ts:30-31` | `app.listen({port: Number(PORT||3000)})`; no host bind (defaults 0.0.0.0 in Fastify). |
| `WEBSITE_URL` | `https://revokers.com/support` | **never read in code** | Dead var — keep for compat or drop. |

No secrets in code; deploy secrets are `SSH_*` in `ci.yml` only.

**Behavior to preserve:** startup must `exec DDL + migration + PRAGMAs` before listening (`src/db.ts:10-79`).

---

## 5. Database

### 5.1 DDL (authoritative — `src/db.ts:11-45`)

```sql
CREATE TABLE IF NOT EXISTS items (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  seller   TEXT NOT NULL,          -- PLAYER NAME, not UUID (see quirk)
  material TEXT,
  amount   INTEGER,                -- 1..64
  price    TEXT,                   -- per-unit, stored as TEXT, CAST to REAL on read
  source   TEXT,                   -- player|creative|command|admin|unknown
  origin   TEXT,                   -- survival|creative|command|unknown
  status   TEXT NOT NULL DEFAULT 'AVAILABLE' -- AVAILABLE|PENDING|SOLD|REMOVED
);
CREATE TABLE IF NOT EXISTS history (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  seller   TEXT,   -- name
  buyer    TEXT,   -- UUID (unlike items.seller)
  material TEXT,
  price    TEXT,
  amount   INTEGER,
  source   TEXT,
  origin   TEXT
);
CREATE TABLE IF NOT EXISTS users (
  playeruuid    TEXT PRIMARY KEY,  -- UUID canonical
  username      TEXT NOT NULL,
  balance       REAL NOT NULL DEFAULT 0,
  password_hash TEXT,              -- nullable; Bun.password format
  created_at    INTEGER DEFAULT (unixepoch())
);
CREATE INDEX IF NOT EXISTS idx_items_status  ON items(status);
CREATE INDEX IF NOT EXISTS idx_items_seller  ON items(seller);
CREATE INDEX IF NOT EXISTS idx_history_buyer ON history(buyer);
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON; -- no FKs declared; kept for future
```

### 5.2 Migration quirk (`src/db.ts:47-79`)

Legacy DB had `users(username TEXT PRIMARY KEY)`. On boot, if detected, migrates to `users_new(playeruuid PK)` via `hex(randomblob(16))` for missing UUIDs. Target language must handle **both** opening a fresh DB and a legacy file gracefully — run the same `IF username PK` check or ship a one-off migration script.

### 5.3 Data quirks (must preserve)

- `items.price` is `TEXT` but always `CAST(price AS REAL)` on read (`src/services/items/items.ts:6`). Writes: legacy `/items/add` passes raw string `price`, modern `/list` does `String(p)` (`src/routes/auction.ts:38`). Target should accept both but store canonically as TEXT or REAL.
- `items.seller` is **name**, not UUID. All UUID→name resolution is via `SELECT username FROM users WHERE playeruuid=$1` (`src/services/items/items.ts:5`, `src/routes/items.ts:138-140`). `history.buyer` is UUID; `history.seller` is name. Keep this asymmetry or migrate sellers to UUID (requires backfill).
- No FK constraints, no `NOT NULL` on most columns.
- `pool` shim translates `$1,$2` placeholders to SQLite positional args; if switching to `?`, keep translation layer.

---

## 6. Global Mutation Version (Long-Polling)

```ts
// src/db.ts:81-85
export const mutationVersion = { value: 0 };
export const bumpVersion = () => { mutationVersion.value += 1; };
export const getVersion  = () => mutationVersion.value;
```
- Bumped on **every** mutation: `INSERT items` (`/items/add` `src/routes/items.ts:106`, `/list` `src/routes/auction.ts:40`), `buy` (`src/routes/items.ts:189`), `retrieve` (`src/routes/items.ts:231`).
- Read into every envelope as `version` (`src/services/items/items.ts:14`).
- **In-memory only** — resets on restart, breaks multi-instance. Target should replace with durable counter (DB `meta` table, sequence, or Redis atomic increment) if horizontal scaling is needed. Poll loop (`src/routes/items.ts:40-55`) depends on it.

---

## 7. Provenance / Anti-Dupe Gate (`src/utils/provenance.ts`)

Prevents creative/admin/command items from entering the economy — checked **twice**: on write (route) and on read (SQL filter).

```ts
// src/utils/provenance.ts:7-17
normalizeSource(raw): "player"|"creative"|"command"|"admin"|"unknown" // default "player"
normalizeOrigin(raw): "survival"|"creative"|"command"|"unknown"       // default "survival"
isBlocked(s,o): BLOCKED_SOURCES={"creative","admin","command"} || BLOCKED_ORIGINS={"creative","command","unknown"}
```

- Routes reject with `400 {success:false, message:"blocked origin: ... (source=...)"}` (`src/routes/items.ts:96`, `src/routes/auction.ts:29`).
- Feed queries add defense-in-depth SQL: `AND LOWER(COALESCE(source,'player')) NOT IN ('creative','admin','command') AND LOWER(COALESCE(origin,'survival')) NOT IN ('creative','command','unknown')` (`src/services/items/items.ts:21,113`).
- **Must port exactly** — bypass is a dupe vector.

---

## 8. Auth Module

### 8.1 Schema & helpers (`src/services/auth/auth.ts`)

```ts
type AuthUser = { playeruuid, username, password_hash: string|null }
getUserByUuid(uuid) -> row|null
getUserByName(name) -> row|null
ensureUser(uuid, username) // INSERT ... ON CONFLICT DO UPDATE username
setPasswordHash(uuid, hash)
isRegistered(uuid) // has password_hash
```

### 8.2 Route behavior (`src/routes/auth.ts`)

| Endpoint | Method | Validates | Logic |
|---|---|---|---|
| `/auth/status?player=uuid` | GET | UUID regex | Returns `{registered:false,hasPassword:false}` if no row; else `{registered:!!hash, hasPassword:!!hash, username}` |
| `/auth/register` | POST | UUID, name (no spaces, len≥2), password 4..64 | Rejects `400 already registered` if hash exists; `ensureUser` then `Bun.password.hash` + `setPasswordHash`; `200 {success:true}` |
| `/auth/login` | POST | UUID, password | In-memory rate limit **5 fails / 60s per `uuid:ip`** (`failMap`, `checkRate`, `recordFail` `src/routes/auth.ts:11-23`); `404 not registered`, `401 invalid password`, `429 too many attempts`; on success clears counter and `200 {success:true, message:"authenticated", username}` — **NO token/session/JWT issued** |
| `/auth/change` | POST | UUID, newPassword 4..64 | `404 not registered`, `401 invalid old password` (verifies `oldPassword` if provided else always fail); hashes new password |

- `UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i` — duplicated in every route file.
- Rate limit is **process-local Map** — resets on restart, not shared.

### 8.3 Migration decision

Current auth is **not used as a guard** — all other endpoints are **unauthenticated**. Target must choose:
  - (a) keep behavior (zero auth) for drop-in compat, or
  - (b) introduce real auth (JWT/session) and protect `/balance/set`, `/shop/catalog/reload`, economy mutations. If (b), keep login/register contract but add `Authorization` header issuance.

### 8.4 Password hash compat

`Bun.password` defaults to **Argon2id**. If target uses different default (bcrypt), existing hashes will fail. Either use an Argon2id lib or force password reset migration.

---

## 9. Balance / Economy (`src/services/balance/balance.ts`, `src/routes/balance.ts`)

### 9.1 Helpers

```ts
registerIfAbsentByUuid(uuid, username?) // upsert name, balance 0
getBalance(uuid): number                // auto-registers at 0 if missing
setBalance(uuid, balance): number       // requires balance>=0, auto-register, UPDATE
changeBalance(uuid, delta): number      // getBalance + check next>=0 else throw "insufficient funds"; UPDATE; returns next — NOT atomic (read-modify-write race)
creditIfRegistered(sellerName, amount)  // lookup uuid by username, then changeBalance
```

Also `registerIfAbsent(username, uuid)` (`src/utils/registerIfAbsent.ts`) — legacy reversed-arg helper doing same upsert.

### 9.2 Endpoints

| Endpoint | Validates | Logic |
|---|---|---|
| `GET /balance?player=uuid` (`src/routes/balance.ts:10`) | UUID required else 400 | `getBalance` → `{playerUuid, balance}` (always 200) |
| `POST /balance/set` | UUID, `balance=Number(...)` finite & ≥0 | `registerIfAbsentByUuid` + `setBalance` → `{success:true, balance:b}` |
| `POST /balance/change` | UUID, `delta=Number(...)` finite | `registerIfAbsentByUuid` + `changeBalance` → `{success:true, balance}` or `400 {success:false, message:"insufficient funds"}` |

**Atomicity gap:** `changeBalance` is two queries (`SELECT` then `UPDATE`) with no `BEGIN`. Concurrent buys can race. Target should use `UPDATE users SET balance = balance + $1 WHERE playeruuid=$2 AND balance+$1>=0 RETURNING balance` or a transaction.

---

## 10. All HTTP Endpoints (Complete Table)

**Base URL:** `http://<host>:<PORT>` (`PORT` default 3000, `.env` sets 8080). All JSON. Prices are **per-unit**. Player IDs are UUID strings unless noted. No auth required on any endpoint today.

| # | Method | Path | File | Query / Body | Auth | Success | Errors |
|---|---|---|---|---|---|---|---|
| 0 | GET | `/` | `src/index.ts:12` | — | none | `200 {status:200, message:"Welcome to Fastify"}` | — |
| 1 | GET | `/stats?player=uuid` | `src/index.ts:16` | `player` (uuid, required) | none | `200 {totalListings, activeListings, totalSold, myListings, mySold, myPurchases}` | `400 {success:false, message:"player query param is required"}` |
| 2 | GET | `/items?page&pageSize&player` | `src/routes/items.ts:32` | `page`=0 fallback 0, `pageSize`=45 fallback 45, `player`=uuid optional | none | `200 envelope` (see §11) | — |
| 3 | GET | `/items/poll?page&pageSize&player&since&timeout` | `src/routes/items.ts:40` | as above + `since`=int default 0, `timeout`=sec clamped 1..60 default 25 | none | `200 envelope` (busy-waits 500ms until `version>since` or deadline) | — |
| 4 | GET | `/items/active?player&...` | `src/routes/items.ts:57` | `player` required, `page/pageSize` | none | `200 envelope` where `seller` matches UUID→username or name | `400 player query param is required` |
| 5 | GET | `/items/bought?player&...` | `src/routes/items.ts:66` | `player` required | none | `200 envelope` from `history` where `buyer=player` | `400 ...` |
| 6 | GET | `/items/history?player&...` | `src/routes/items.ts:75` | `player` required | none | `200 envelope` from `history` where `seller=player OR buyer=player OR seller IN (SELECT username WHERE playeruuid=player)` | `400 ...` |
| 7 | POST | `/items/add` | `src/routes/items.ts:84` | `ItemsAddBody` (legacy strings — see §10.1) | none | `200 {success:true, message:"item created with id : <id>"}` + bump version | `400` price len>20, name spaces, material `^[A-Z0-9_]+$`, amount 1..64, blocked provenance; `500 unexpected` |
| 8 | POST | `/list` | `src/routes/auction.ts:13` | modern numeric body — see §10.1 | none | `200 {success:true, message:"item created with id : <id>"}` + bump | `400` UUID/name/material/amount 1..64/price>0/blocked |
| 9 | POST | `/items/buy` | `src/routes/items.ts:114` | `{wantedId:number, buyerId:uuid}` | none | `200 {success:true, balance:<newBuyerBalance>, message:"item added with id : <historyId>"}` + bump | `400` invalid wantedId/malformed UUID, `404 item not found`, `400 you cannot buy your own listing`, `400 item is processing... (PENDING)`, `400 item sold out`, `400 insufficient funds`, `500` |
| 10 | POST | `/items/retrieve` | `src/routes/items.ts:205` | `{wantedId:number, sellerId:uuid}` | none | `200 {success:true, material, amount, message:"item <id> retrieved"}` status→`REMOVED` + bump | `400 invalid wantedId/malformed UUID`, `404 item not found`, `400 item is not available to retrieve` (not AVAILABLE), `400 you can only retrieve your own listings` |
| 11 | GET | `/balance?player=uuid` | `src/routes/balance.ts:10` | `player=uuid` | none | `200 {playerUuid, balance}` | `400 player query param is required` |
| 12 | POST | `/balance/set` | `src/routes/balance.ts:17` | `{playerUuid, playerName?, balance:number}` | none | `200 {success:true, balance}` | `400 malformed UUID / invalid balance (<0 or NaN)` |
| 13 | POST | `/balance/change` | `src/routes/balance.ts:27` | `{playerUuid, playerName?, delta:number}` | none | `200 {success:true, balance}` | `400 malformed UUID / invalid delta / insufficient funds` |
| 14 | GET | `/auth/status?player=uuid` | `src/routes/auth.ts:27` | `player=uuid` | none | `200 {registered, hasPassword, username?}` | `400 player query param required (uuid)` |
| 15 | POST | `/auth/register` | `src/routes/auth.ts:36` | `{playerUuid, playerName, password 4..64}` | none | `200 {success:true, message:"registered"}` | `400 malformed UUID / invalid playerName / password 4-64 / already registered` |
| 16 | POST | `/auth/login` | `src/routes/auth.ts:52` | `{playerUuid, password}` | none (rate-limited) | `200 {success:true, message:"authenticated", username}` | `400 malformed UUID/password required`, `404 not registered`, `401 invalid password`, `429 too many attempts` |
| 17 | POST | `/auth/change` | `src/routes/auth.ts:69` | `{playerUuid, oldPassword, newPassword 4..64}` | none | `200 {success:true, message:"password changed"}` | `400 malformed UUID/new password 4..64`, `404 not registered`, `401 invalid old password` |
| 18 | GET | `/shop/catalog` | `src/routes/shop.ts:24` | — | none | `200 {version:number, lastUpdated:ISO|null, items:[{id,material,category,buy,sell}]}` | `500 shop catalog unavailable` |
| 19 | POST | `/shop/catalog/reload` | `src/routes/shop.ts:39` | — | none (!) | `200 {success:true, message:"reloaded"}` | `500 reload failed` |

### 10.1 Listing create — two shapes (both must be supported)

**Legacy `POST /items/add`** (`src/routes/items.ts:84-112`, `src/types/items/items.ts:1-9`):
```json
{
  "playeruuid": "uuid",
  "playerName": "Steve",
  "material": "DIAMOND",
  "amount": "3",
  "price": "1500.0",
  "source": "player",
  "origin": "survival"
}
```
- `amount` and `price` are **strings**; code does `amount.trim()` → `Number()`, validates `amt 1..64`, `price.length>20` reject, `materialRe /^[A-Z0-9_]+$/`, `playerName.trim()` no `\s`.
- Calls `registerIfAbsent(playerName, playeruuid)` (fire-and-forget, not awaited).

**Modern `POST /list`** (`src/routes/auction.ts:13-46`):
```json
{
  "playerUuid": "uuid",
  "playerName": "Steve",
  "material": "DIAMOND",
  "amount": 3,
  "price": 1500.0,
  "source": "player",
  "origin": "survival"
}
```
- Numeric `amount`/`price`; validates `amt 1..64`, `price>0 && finite`, same material/name checks.
- Note key casing: `playerUuid` vs `playeruuid`.

Both normalize `source`/`origin` and enforce `isBlocked`, then `INSERT INTO items (...) VALUES ($1..$6,'AVAILABLE') RETURNING id`.

---

## 11. Envelope & Pagination

```json
{
  "page": 0,
  "pageSize": 45,
  "total": 214,
  "totalPages": 5,
  "version": 7,
  "items": [{ "id": 12, "sellerName": "Steve", "material": "DIAMOND", "amount": 3, "price": 1500.0 }]
}
```
- Helper `envelopeData` (`src/services/items/items.ts:8-15`): `totalPages = Math.ceil(total / pageSize)`, `version = getVersion()`.
- `page` is **0-based** (`offset = page*pageSize`, `src/services/items/items.ts:18,49`); plugin sends `page=0` for first page; parser clamps negatives to fallback (`parsePage` `src/routes/items.ts:19-22`).
- `price` is `CAST(price AS REAL)` aliased to `price`; `seller` aliased to `sellerName`.

**List queries:**

- **Global feed** `getItems` (`src/services/items/items.ts:17-46`): `WHERE status='AVAILABLE'` + provenance filter; if `excludePlayer` given adds `AND seller NOT IN (SELECT username FROM users WHERE playeruuid=$1) AND seller <> $1`. `ORDER BY id` limited.
- **By seller** `getBySeller` (`src/services/items/items.ts:48-68`): `WHERE status='AVAILABLE' AND (seller=$1 OR seller IN (SELECT username WHERE playeruuid=$1))` — so UUID param matches both name and resolved name.
- **By buyer** `getByBuyer` (`src/services/items/items.ts:70-87`): `history WHERE buyer=$1 ORDER BY id DESC`.
- **History** `getHistory` (`src/services/items/items.ts:89-110`): `history WHERE seller=$1 OR buyer=$1 OR seller IN (SELECT username WHERE playeruuid=$1)`.
- **Stats** `getStats` (`src/services/items/items.ts:112-131`): 6 counts in parallel.

---

## 12. Business Logic — Buy & Retrieve Flows

### 12.1 Buy (`POST /items/buy` — `src/routes/items.ts:114-203`)

```
validate wantedId finite & buyerId UUID
SELECT * FROM items WHERE id=$1 -> 404 if missing
if status != AVAILABLE -> 400 (PENDING => "processing", else "sold out")
ownerUuid = SELECT playeruuid FROM users WHERE username=listing.seller
if ownerUuid == buyerId -> 400 "you cannot buy your own listing"
cost = Number(price)*amount; buyerBalance = getBalance(buyerId)
if buyerBalance < cost -> 400 "insufficient funds"

BEGIN
  UPDATE items SET status='PENDING' WHERE id=$1 AND status='AVAILABLE' RETURNING *
  if 0 rows -> ROLLBACK, 400 "sold out"          // lost race
  re-check own-listing (SELECT result) -> ROLLBACK if violated
  UPDATE items SET status='SOLD' WHERE id=$1
  INSERT INTO history (seller,buyer,material,price,amount,source,origin) RETURNING id
COMMIT

changeBalance(buyerId, -cost)          // outside tx; returns newBalance
if ownerUuid: try changeBalance(ownerUuid, +cost) catch swallow  // best-effort seller credit
bumpVersion()
return 200 {success:true, balance:newBalance, message:"item added with id : <historyId>"}
on error: ROLLBACK, 500
```

**Critical migration fixes suggested:**
- Move both balance mutations **inside the transaction** with atomic `UPDATE ... SET balance=balance+delta WHERE ... AND balance+delta>=0`.
- Use a single `UPDATE items SET status='SOLD' WHERE id=$1 AND status='AVAILABLE'` instead of PENDING→SOLD two-step, or add row-level locking (`FOR UPDATE`).
- Debit/credit currently **not atomic** with the sale — crash between COMMIT and credit loses seller money. Credit failure is swallowed.

### 12.2 Retrieve (`POST /items/retrieve` — `src/routes/items.ts:205-238`)

```
validate wantedId & sellerId UUID
SELECT * FROM items WHERE id=$1 -> 404
if status != AVAILABLE -> 400 "item is not available to retrieve"
ownerUuid = SELECT playeruuid FROM users WHERE username=seller
if !ownerUuid || ownerUuid != sellerId -> 400 "you can only retrieve your own listings"
UPDATE items SET status='REMOVED' WHERE id=$1   // not in tx, single statement
bumpVersion()
return 200 {success:true, material, amount, message:"item <id> retrieved"}
```

Plugin uses returned `material`/`amount` to rebuild the `ItemStack`.

---

## 13. Shop Catalog (`src/routes/shop.ts`, `src/data/items.json`)

- File `src/data/items.json`: `{ items: [{id, material, category, buy, sell}, ...] }` — `buy`/`sell` are per-unit prices for the shop (not AH).
- Loading (`src/routes/shop.ts:8-20`): `fs.readFileSync` + `JSON.parse`, cached by `mtimeMs`; `cached.mtime` check.
- `GET /shop/catalog` returns `{version: floor(mtimeMs/1000), lastUpdated: ISO, ...data}`.
- `POST /shop/catalog/reload` clears `cached=null` then reloads — unauthenticated admin hot-reload. Target should add auth.
- `balance/change` is used by the shop to move money (negative `delta` = buy from shop, positive = sell).

---

## 14. Validation & Error Conventions

- **UUID:** `UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i` — duplicated per route file.
- **Material:** `materialRe = /^[A-Z0-9_]+$/` on `material.trim()`; must be uppercase.
- **Amount:** `Number(amount)` finite, `1..64` inclusive (validated on both listing endpoints; `amount` string trimmed for legacy).
- **Price:** legacy checks `price.length>20`; modern checks `Number(price)>0 && finite`. Cost = `price * amount`.
- **Player name:** `playerName.trim()` must not contain `\s` (regex `/\s/`), and for auth `len≥2`.
- **Password:** `4..64` chars.
- **Pagination:** `parsePage(raw, fallback)` — `Number(raw)` floored, clamped ≥0 else fallback.
- **Errors:** `fail(res, code, message) => res.code(code).send({success:false, message})` (`src/routes/items.ts:26-27`). Legacy root routes use `{error}` envelope but current code is `success/message`. Pick one; keep `success:false` for plugin compat.
- **No schema validation, no Fastify hooks, no middleware, no global error handler** — all inline `try/catch`.

---

## 15. Concurrency, Timeouts & Polling

| Concern | Current | Target advice |
|---|---|---|
| Long-poll `/items/poll` | Busy-wait `sleep(500)` loop until `version>since` or deadline (`Math.min(Math.max(timeout||25,1),60)*1000`) (`src/routes/items.ts:40-55`) | Replace with event/notify, `SETUP` pub/sub, or DB `LISTEN/NOTIFY`; avoid 500ms spin |
| Global version | In-memory counter — reset/lost on restart, single-instance | `meta(version)` row with `UPDATE meta SET version=version+1 RETURNING version` |
| Buy race | `UPDATE ... WHERE status='AVAILABLE' RETURNING` is the linearization point; 0 rows → sold out | Keep pattern; add `SELECT FOR UPDATE` if using Postgres |
| Balance race | `getBalance` then `UPDATE` separate — lost-update | `UPDATE ... SET balance=balance+$delta WHERE balance+$delta>=0 RETURNING balance` — check `rowCount==0` → insufficient funds |
| SQLite locking | Single writer; `busy_timeout` not set | Set `PRAGMA busy_timeout=5000` or use WAL properly |

---

## 16. Security Notes (behavior to preserve or intentionally change)

- **No auth guard** on any economy/shop endpoint — `/balance/set` and `/shop/catalog/reload` are effectively admin and currently open. Migration should add authz (roles) but keep an **unauthenticated mode** flag if plugin must keep working without tokens during rollout.
- Rate limiting only on `POST /auth/login` (`src/routes/auth.ts:10-23`) — in-memory, per `uuid:ip`.
- CORS not configured — plugin is same-network; add if web frontend needs it.

---

## 17. Tests as Contract (port these first)

`test/api.test.ts` is the executable spec:

1. `POST /list {DIAMOND, amount 1, price 100}` → `success:true`
2. `POST /list {material:"diamond"}` → `success:false` (lowercase reject)
3. `GET /items?page=0&pageSize=45` → `page==0, items[0].sellerName=="Steve", price==100` (0-based envelope)
4. `GET /items/active?player=<uuid>` → `total>0`
5. `POST /items/buy` own listing → `400 you cannot buy your own listing`
6. Fund `/balance/change delta:10000` → buy as other UUID → `200 success:true`; then `GET /items/bought?player=...` and `/items/history?player=...` → `total>0`
7. `POST /items/buy wantedId:999999` → `404`

Use these as **contract tests** in the target language before other work.

---

## 18. Deployment & Ops

- **CI** `.github/workflows/ci.yml`: `bun install` + `bun test` on PRs; on push to `master` SCP `package.json bun.lock .env.example src test deploy` to host and `bash deploy/remote.sh`.
- **Service** `deploy/bazzar.service`: `WorkingDirectory=/var/www/bazzar`, `EnvironmentFile=/var/www/bazzar/.env`, `ExecStart=/root/.bun/bin/bun run src/index.ts`, `Restart=on-failure`.
- **Health check:** remote.sh expects service restart + `systemctl` health. Keep or add `GET /` + `/stats` probes.

---

## 19. What to Implement in the Target Language (Checklist)

### MVP (drop-in replacement)

- [ ] Config from `DATABASE_URL`, `PORT` (fallback 3000), keep `WEBSITE_URL` compat.
- [ ] SQLite open + DDL + migration check + `WAL` + `foreign_keys=ON` + pool shim or native queries.
- [ ] In-memory (or durable) `version` + `bumpVersion`/`getVersion` + envelope `{page,pageSize,total,totalPages,version,items}`.
- [ ] All 20 endpoints exactly as in §10 with same status codes/messages/payload keys (`sellerName`, `message` strings `you cannot buy your own listing`, `item is processing...`, `insufficient funds`, `item created with id : X`, etc.).
- [ ] Both listing shapes (`/items/add` string fields + `/list` numeric) + `registerIfAbsent` upsert.
- [ ] Provenance `normalize*` + `isBlocked` + SQL provenance filters.
- [ ] Buy tx: `PENDING` guard + `SOLD` + `history` insert (+ best-effort credits or fix to atomic).
- [ ] Balance: `get/set/change` with `insufficient funds` check.
- [ ] Auth: `status/register/login/change` with Argon2id + `5/60s` rate limit (or better) — no JWT required for MVP.
- [ ] Shop catalog: `items.json` mtime-cache + `GET /shop/catalog` + `POST /shop/catalog/reload`.
- [ ] `GET /` welcome + `GET /stats` counts.
- [ ] `GET /items/poll` long-poll (clamped 1..60s, 500ms wake).
- [ ] Pagination: 0-based, `totalPages = ceil(total/pageSize)`.
- [ ] Validation regexes and error envelope `{success:false, message:String}`.

### Hardening (recommended post-MVP)

- [ ] Replace in-memory version with durable counter; replace sleep-poll with notify/pubsub.
- [ ] Make `changeBalance` / buy credit **atomic** (`UPDATE ... SET balance=balance+delta WHERE ... RETURNING`).
- [ ] Add real auth (JWT) + protect admin endpoints (`/balance/set`, `/shop/catalog/reload`).
- [ ] Add `PRAGMA busy_timeout`, proper connection pool, request schema validation, global error handler.
- [ ] Switch `items.seller` to UUID (backfill) to eliminate name↔uuid join.
- [ ] Normalize `price` column to `REAL` and add indexes as needed.

---

## 20. Key Files to Read When Porting

```
src/db.ts:1-98              DDL, migration, pool shim, version counter
src/index.ts:1-38            Route registration & GET /stats wire-up
src/routes/items.ts:1-239    Largest route — all item feeds, buy, retrieve, provenance
src/routes/auction.ts:1-47   Modern listing create (numeric)
src/routes/balance.ts:1-40   Balance CRUD
src/routes/auth.ts:1-81      Auth + rate limit
src/routes/shop.ts:1-48      Catalog cache
src/services/items/items.ts:1-131  Envelope + SQL for all list/history/stats queries
src/services/balance/balance.ts:1-47  Balance helpers & auto-register
src/services/auth/auth.ts:1-34       User lookups
src/utils/provenance.ts:1-31         Block logic (must replicate exactly)
src/utils/registerIfAbsent.ts:1-13   Upsert helper
src/types/items/items.ts:1-9         Legacy body type
src/data/items.json          Shop catalog shape (~300 entries)
test/api.test.ts:1-103       Contract tests — port first
api.md:1-287                 Human-readable API contract
```

---

> Tip: Start the port by copying `test/api.test.ts` into the target language's test suite and making it pass before adding new features.
