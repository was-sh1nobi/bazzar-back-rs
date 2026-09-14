# Bazzar Backend — Migration Guide (Language-Agnostic Spec)

> **MC-ONLY rebuild** — Single source of truth for porting the **Minecraft-facing backend only**. Frontend (`/ui/*`, dashboard, `web_users`) is stripped — you will rebuild it by hand. Everything below is MC endpoints, logic, DB, auth, quirks, and **2026-08-30 zero-trust shop refactor**.
> Covers codebase at `src/` + `api.md` + `.env.example` + tests. Two backends exist: `WebstormProjects/bazzar-back` (Node/Bun/Fastify — legacy) and `RustroverProjects/bazzar-back` (Rust/Actix/SeaORM — canonical, JWT Bearer per-user, in-memory). This guide is the contract both must satisfy.

---

## 1. Overview

**Bazzar** is a Minecraft Auction House (AH) + server-side economy backend consumed by a Paper/Spigot plugin (`untitled/`, `com.revokers.bazzar`) over HTTP (off main thread, 30s client timeout). Money has **no client state** — all balances live server-side. Listings are `ItemStack`s held in hand; the plugin removes from hand before calling the backend and refunds on failure.

```
Minecraft Plugin (Java)  --HTTP JSON (BackendClient.java:26)-->  Bazzar Backend (Fastify)
                                                                   |-- SQLite (bazzar.db) WAL + FK ON
                                                                   |-- src/data/items.json (canonical catalog file)
                                                                   |-- shop_items + shop_purchases tables (authoritative)
```

**Trust model (post-2026-08-30, updated 2026-08-31 JWT):** Plugin is **untrusted**. All price math, balance checks, and deductions happen server-side. Plugin only sends `{itemId, amount}` + `Authorization: Bearer <JWT>`; backend extracts `sub` (playerUuid) from verified JWT (`HS256`, `iss=bazzar-back`, `aud=bazzar-client`, `exp` 24h + 60s leeway, `jti` uuid) stored only in server memory (Rust `auth.rs:20` `JWT_SECRET`, `TTL_SECONDS=86400`). No `delta` or `playerUuid` in body is trusted — `auth.claims.sub` is canonical. Node legacy still accepts `playerUuid` in body but must migrate to Bearer.

**Plugin flow for shop purchases (ShopService.java:30, BackendClient.java:174):**
```
plugin: GET /balance?player=UUID          -> optimistic pre-check + UI
plugin: POST /shop/buy {playerUuid, itemId, amount}  -> backend validates & deducts atomically
backend: SELECT buy FROM shop_items WHERE id=$1 -> cost = buy*amount
backend: UPDATE users SET balance=balance-cost WHERE playeruuid=$1 AND balance>=cost RETURNING balance
plugin: on success -> addItem to inventory (main thread), show buySuccess
```

---

## 2. Stack to Replicate

| Concern | Current (Node) | Target requirement |
|---|---|---|
| Runtime | **Bun** (`bun:sqlite`, `Bun.password`, `bun test`) | Any runtime; replace with native driver + hashing |
| Language | TypeScript (no `tsconfig`) | Target language |
| HTTP | **Fastify v5** (`src/index.ts:14`) | Any framework (Gin, Spring, Axum, Actix, FastAPI…) |
| DB | **SQLite WAL** file `./bazzar.db`, `PRAGMA journal_mode=WAL`, `PRAGMA foreign_keys=ON` (`src/db.ts:7-8`) | Keep SQLite or migrate to Postgres; preserve schema+semantics |
| ORM | None — raw SQL via fake `pool` shim (`src/db.ts:131-142`) mimics `pg` (`query`→`{rows}`, `connect()`→`BEGIN/COMMIT`) with `$1,$2` placeholders → rewrite to `?` internally | Use real driver/ORM; keep `$1` API or translate |
| Password hashing | `Bun.password.hash/verify` (Argon2id default) (`src/routes/auth.ts:46`) / Rust `argon2` crate (`password_verifier.rs:12`) | `argon2id` — must stay compatible; otherwise force password reset |
| Auth | Node: shared `PLUGIN_SECRET` header `x-plugin-secret` (`pluginAuth.ts`) — legacy | **Canonical: per-user JWT Bearer** (`Rust: src/auth.rs:180` `Auth` extractor, `create_jwt` HS256, `JWT_SECRET` >=32, 24h TTL, in-memory only, `jti` uuid) |
| Tests | `bun test` + Fastify `inject()` (`test/api.test.ts`, `test/ui.test.ts`) | Port tests to target framework's test client |
| Logging | `Fastify({logger:true})` + `console.*` | Structured logger |
| Deploy | systemd `deploy/bazzar.service`, `deploy/remote.sh`, GitHub Actions `ci.yml` (`bun test` + SCP) | Adapt |
| Shop catalog | File `src/data/items.json` + DB `shop_items` table (authoritative) | Must seed DB from file on boot |

**Scripts:** `dev` = `bun run --watch src/index.ts`, `test` = `bun test` (`package.json:2-5`).

---

## 3. Project Structure (what each file does)

```
src/index.ts                    App bootstrap, registers route plugins, GET / + GET /stats + security/rateLimit/banGuard
src/db.ts                       SQLite open + DDL (items/history/users/web_users/shop_items/shop_purchases) + migration + pool shim + global version counter
src/routes/items.ts             GET /items, /items/poll, /items/active, /items/bought, /items/history
                                POST /items/add, /items/buy, /items/retrieve
src/routes/auction.ts           POST /list  (modern listing create)
src/routes/balance.ts           GET /balance  (writes are private — no POST /balance/*)
src/routes/auth.ts              GET /auth/status, POST /auth/register|login|change (rate-limited)
src/routes/shop.ts              GET /shop/catalog, POST /shop/catalog/reload, POST /shop/buy, POST /shop/sell  ★ NEW
src/services/items/items.ts     getItems, getBySeller, getByBuyer, getHistory, getStats + envelope logic
src/services/balance/balance.ts registerIfAbsentByUuid, getBalance, setBalance, changeBalance (ATOMIC), deductBalanceAtomic, creditIfRegistered
src/services/shop/catalog.ts    seedShopItemsIfNeeded, getShopItemById/Material, reloadShopItemsFromFile  ★ NEW
src/services/auth/auth.ts       getUserByUuid/Name, ensureUser, setPasswordHash, isRegistered
src/utils/provenance.ts         normalizeSource/Origin, isBlocked — anti-dupe gate
src/utils/registerIfAbsent.ts   INSERT ... ON CONFLICT DO UPDATE username (legacy helper)
src/types/items/items.ts        ItemsAddBody (legacy string fields)
src/data/items.json             Static shop catalog (~270 entries, ~36KB) — canonical file source
test/api.test.ts                Canonical AH integration tests (use as contract tests)
deploy/bazzar.service           systemd unit
deploy/remote.sh                Remote deploy script
.github/workflows/ci.yml        CI: bun install/test; deploy on push master
api.md                          Authoritative API contract (MC-only)
```

---

## 4. Configuration & Environment

| Var | Default | Where used | Notes |
|---|---|---|---|
| `DATABASE_URL` | `./bazzar.db` | `src/db.ts:3` | SQLite file path. Target can use `DATABASE_URL` as DSN. Rust port uses Postgres `DATABASE_URL`. |
| `PORT` | `3000` (`.env`/`api.md` use `8080`) | `src/index.ts:55` via `intEnv("PORT",3000)` | `app.listen({port, host})`; host from `HOST` env default `0.0.0.0`. |
| `HOST` | `0.0.0.0` | `src/index.ts:57` via `strEnv("HOST")` | Bind host. |
| `TRUST_PROXY` | `false` | `src/index.ts:19` | Only trust `X-Forwarded-*` when behind reverse proxy (rateLimit uses `req.ip`). |
| `NODE_ENV` | `development` | `src/utils/config.ts:7` | `production` enables prod-mode checks. |
| `WEBSITE_URL` | `https://revokers.com/support` | **never read in code** | Dead var — keep for compat or drop. |

No secrets in code; deploy secrets are `SSH_*` in `ci.yml` only.

**Behavior to preserve:** startup must `exec DDL + migration + PRAGMAs` before listening (`src/db.ts:10-123`). Shop seed (`seedShopItemsIfNeeded`) runs on first `GET /shop/catalog` or `POST /shop/*` (`src/routes/shop.ts:30`).

---

## 5. Database

### 5.1 DDL (authoritative — `src/db.ts:10-79`, updated 2026-08-30)

```sql
CREATE TABLE IF NOT EXISTS items (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  seller   TEXT NOT NULL,          -- PLAYER NAME, not UUID (see quirk §5.3)
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
  password_hash TEXT,              -- nullable; Bun.password Argon2id format
  created_at    INTEGER DEFAULT (unixepoch()),
  banned        INTEGER NOT NULL DEFAULT 0   -- added by migration, see §5.2
);

-- UI-ONLY (stripped for MC-only backend) — kept for reference but not required if you rebuild frontend:
-- CREATE TABLE IF NOT EXISTS web_users (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  username TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL,
  role     TEXT NOT NULL DEFAULT 'member',
  created_at INTEGER DEFAULT (unixepoch())
);

-- ★ NEW 2026-08-30: authoritative shop catalog (price source of truth)
CREATE TABLE IF NOT EXISTS shop_items (
  id       TEXT PRIMARY KEY,       -- e.g. "diamond", "stone"
  material TEXT NOT NULL UNIQUE,   -- e.g. "DIAMOND"
  category TEXT NOT NULL,          -- e.g. "ORES"
  buy      REAL NOT NULL CHECK(buy >= 0),
  sell     REAL NOT NULL CHECK(sell >= 0)
);

-- ★ NEW 2026-08-30: audit log for shop purchases
CREATE TABLE IF NOT EXISTS shop_purchases (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  playeruuid TEXT NOT NULL,
  itemId     TEXT NOT NULL,
  material   TEXT NOT NULL,
  amount     INTEGER NOT NULL,
  priceEach  REAL NOT NULL,
  totalCost  REAL NOT NULL,
  created_at INTEGER DEFAULT (unixepoch()),
  FOREIGN KEY(playeruuid) REFERENCES users(playeruuid)
);

CREATE INDEX IF NOT EXISTS idx_items_status  ON items(status);
CREATE INDEX IF NOT EXISTS idx_items_seller  ON items(seller);
CREATE INDEX IF NOT EXISTS idx_history_buyer ON history(buyer);
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE INDEX IF NOT EXISTS idx_web_users_username ON web_users(username);
CREATE INDEX IF NOT EXISTS idx_shop_material ON shop_items(material);
CREATE INDEX IF NOT EXISTS idx_shop_category ON shop_items(category);
CREATE INDEX IF NOT EXISTS idx_shop_purchases_player ON shop_purchases(playeruuid);

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
```

### 5.2 Migration quirk (`src/db.ts:81-123`)

1. Legacy `users(username TEXT PRIMARY KEY)` → `users_new(playeruuid PK)` via `hex(randomblob(16))` for missing UUIDs. Detect by `PRAGMA table_info` + `sql LIKE '%username TEXT PRIMARY KEY%'`.
2. Ensure `users.password_hash`, `created_at`, `banned` columns exist (ALTER TABLE ADD COLUMN if missing).
3. Drop legacy `users.role` column if present (roles moved to `web_users`).
4. **Shop seed:** `seedShopItemsIfNeeded()` (`src/services/shop/catalog.ts:11`) — if `SELECT COUNT(*) FROM shop_items == 0` and `src/data/items.json` exists, bulk `INSERT OR IGNORE` all entries. Runs lazily on first shop route call. `reloadShopItemsFromFile()` does `DELETE + bulk INSERT` for `POST /shop/catalog/reload`.

Target language must handle both fresh DB and legacy file gracefully.

### 5.3 Data quirks (must preserve)

- `items.price` is `TEXT` but always `CAST(price AS REAL)` on read (`src/services/items/items.ts:6`). Writes: legacy `/items/add` passes raw string `price`, modern `/list` does `String(p)` (`src/routes/auction.ts:38`). Store as TEXT or REAL but read as REAL.
- `items.seller` is **name**, not UUID. UUID→name via `SELECT playeruuid FROM users WHERE username=$1` (`src/routes/items.ts:138`). `history.buyer` is UUID; `history.seller` is name.
- No FK constraints on items/history (only `shop_purchases.playeruuid` has FK).
- `pool` shim translates `$1,$2` to SQLite positional args; if switching to `?`, keep translation.

---

## 6. Global Mutation Version (Long-Polling)

```ts
// src/db.ts:125-129
export const mutationVersion = { value: 0 };
export const bumpVersion = () => { mutationVersion.value += 1; };
export const getVersion  = () => mutationVersion.value;
```
- Bumped on every mutation: `INSERT items` (`/items/add` `104`, `/list` `40`), `buy` (`189`), `retrieve` (`231`). Shop buy/sell intentionally do **not** bump version (shop purchases don't appear in AH feeds). If you want live-updates for shop stats, bump there too.
- Read into every envelope as `version` (`src/services/items/items.ts:14`).
- **In-memory only** — resets on restart, breaks multi-instance. Replace with durable counter (`meta` table or Redis) if horizontal scaling.

---

## 7. Provenance / Anti-Dupe Gate (`src/utils/provenance.ts`)

Prevents creative/admin/command items from entering economy — checked **twice**: on write (route) and on read (SQL filter).

```ts
normalizeSource(raw): "player"|"creative"|"command"|"admin"|"unknown" // default "player"
normalizeOrigin(raw): "survival"|"creative"|"command"|"unknown"       // default "survival"
isBlocked(s,o): BLOCKED_SOURCES={"creative","admin","command"} || BLOCKED_ORIGINS={"creative","command","unknown"}
```

- Routes reject `400 {success:false, message:"blocked origin: ... (source=...)"}` (`src/routes/items.ts:96`, `src/routes/auction.ts:29`).
- Feed queries add SQL: `AND LOWER(COALESCE(source,'player')) NOT IN ('creative','admin','command') AND LOWER(COALESCE(origin,'survival')) NOT IN ('creative','command','unknown')` (`src/services/items/items.ts:21`).
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

### 8.2 Route behavior (`src/routes/auth.ts` / Rust `src/controllers/auth.rs` + `src/services/auth.rs`)

| Endpoint | Method | Validates | Logic |
|---|---|---|---|
| `/auth/status?player=uuid` | GET | UUID regex | `{registered:false,hasPassword:false}` if no row; else `{registered:!!hash, hasPassword:!!hash, username}` — **public** |
| `/auth/register` | POST | UUID, name (no spaces, len≥2), password 4..64 | `400 already registered` if hash exists; Argon2id `hash_password` (`password_verifier.rs:12` / `Bun.password.hash`) + `INSERT`; `201 {uuid, username, message:"Created new user"}` |
| `/auth/login` | POST | UUID, password 4..64 | Rate limit **5 fails / 60s per `uuid:ip`**; `404 not registered`, `401 invalid password`, `429 too many attempts`; on success **issues JWT** `create_jwt(sub, username)` (`auth.rs:98`, `services/auth.rs:173`) → `200 {token, token_type:"Bearer", expires_in:86400}` — plugin must store in memory and send `Authorization: Bearer <token>` on every mutating request |
| `/auth/change` | POST | **Requires `Authorization: Bearer <JWT>`** (`Rust: auth: Auth` extractor `controllers/auth.rs:95`); newPassword 4..64 | `401 missing/invalid/expired token` if no Bearer; `404 not registered`, `401 invalid old password`; Argon2 verify old, hash new |

**JWT shape (Rust `src/auth.rs:54` `Claims`):** `sub=playerUuid`, `username`, `iat`, `exp=iat+86400`, `iss=bazzar-back`, `aud=bazzar-client`, `jti=uuidv4`. Signed `HS256` with `JWT_SECRET` (>=32, env `JWT_SECRET`, dev fallback `dev-only-please-change...`). Verified with pinned `HS256`, `exp`+60s leeway, `iss`/`aud`/`sub` required, `none` rejected (`auth.rs:116`). Token lives only in server memory (no DB) + plugin memory (`Map<UUID,token>`); logout not yet implemented (remaining risk). |

- `UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i` — duplicated per route.
- Rate limit is process-local Map — resets on restart.

### 8.3 Migration decision

Current auth is **not a guard** — all other endpoints are unauthenticated. Target: (a) keep zero-auth for compat, or (b) add JWT (Rust port already uses `jsonwebtoken`) and protect `/balance/set`, `/shop/catalog/reload`.

### 8.4 Password hash compat

`Bun.password` defaults to **Argon2id**. If target uses bcrypt, existing hashes fail. Use Argon2id lib or force reset.

---

## 9. Balance / Economy — ★ REWRITTEN 2026-08-30

### 9.1 Helpers (`src/services/balance/balance.ts:8-60`)

```ts
registerIfAbsentByUuid(uuid, username?) // upsert name, balance 0
getBalance(uuid): number                // auto-registers at 0 if missing
setBalance(uuid, balance): number       // requires balance>=0, auto-register
changeBalance(uuid, delta): number      // ★ ATOMIC now: UPDATE ... SET balance=balance+$1 WHERE balance+$1>=0 RETURNING balance; throw "insufficient funds" if 0 rows
deductBalanceAtomic(uuid, cost): number // wrapper: changeBalance(uuid, -cost) with cost>0 check
creditIfRegistered(sellerName, amount)  // lookup uuid by username, then changeBalance
```

**Before 2026-08-30:** `changeBalance` was `getBalance + check + UPDATE` (race). **After:** single atomic `UPDATE ... WHERE balance+$1>=0 RETURNING` (`src/services/balance/balance.ts:32-46`). This is the linearization point for all money moves, including shop `POST /shop/buy|sold` and AH `POST /items/buy` seller credit.

### 9.2 Endpoints (`src/routes/balance.ts:9-40`)

| Endpoint | Validates | Logic |
|---|---|---|
| `GET /balance?player=uuid` | UUID required else 400 | `getBalance` → `{playerUuid, balance}` |
| `POST /balance/set` | — REMOVED | Private only — use DB seeding or internal service | 410 Gone |
| `POST /balance/change` | — REMOVED | Private only — use POST /shop/buy, POST /shop/sell, or POST /items/buy (all price-from-DB, atomic) | 410 Gone |

**Zero-trust note (2026-08-30):** `POST /balance/change` and `POST /balance/set` have been **REMOVED** as public endpoints. Balance mutations are private services only (`POST /shop/buy`, `POST /shop/sell`, `POST /items/buy` all do `UPDATE users SET balance=balance±cost WHERE balance±cost>=0 RETURNING` internally). Plugin only ever sends MATERIAL/itemId+amount.

### 9.3 Flow diagram — shop buy vs legacy

```
Legacy (INSECURE, removed from plugin):
  plugin computes cost = item.buy * amount (from local cache)
  plugin -> POST /balance/change {delta: -cost}   // backend trusts delta

Current (AUTHORITATIVE):
  plugin -> GET /balance?player=uuid              // optimistic UI, optional
  plugin -> POST /shop/buy {playerUuid, itemId, amount}  // only ID+qty sent
  backend: row = SELECT buy FROM shop_items WHERE id=$1 OR material=$1
           cost = row.buy * amount
           UPDATE users SET balance=balance-cost WHERE ... AND balance>=cost RETURNING
           INSERT INTO shop_purchases(...)         // audit
  backend -> {success:true, balance, cost, amount, material, itemId}
  plugin: addItem(material, amount) on main thread
```

---

## 10. All HTTP Endpoints — MC-ONLY (UI `/ui/*` Stripped)

> Frontend rebuild: all `/ui/*`, `/ui/api/*`, dashboard, ban UI, and `web_users` are **out of scope**. Only the endpoints below are required for the Paper plugin. Implement them exactly; add UI later on top.

**Base URL:** `http://<host>:<PORT>` (`PORT` default 3000/8080). All JSON. Prices are **per-unit**. Player IDs are UUID strings.

**Auth model (2026-08-31):** `POST /auth/register|login` and `GET /auth/status|/shop/catalog|GET /` remain **public**. All other endpoints require `Authorization: Bearer <JWT>` (per-user, in-memory, HS256 24h). Legacy Node still accepts `x-plugin-secret` shared secret but is deprecated. Table marks `Auth = Bearer` where JWT required.

**Rust port status (2026-08-30):** **15 / 22 done** — auth (4), stats, all items feeds (`/items`, `/poll`, `/active`, `/bought`, `/history`), `/items/add`, `/list`, `/items/buy`, `/items/retrieve`, `/balance`. Remaining: `GET /` welcome, the two `410 Gone` stubs, and the shop suite (`/shop/catalog`, `/shop/catalog/reload`, `/shop/buy`, `/shop/sell`). Deviations in the port are marked inline (✅ with notes / ⚠). Source: `src/controllers/{auth,balance,stats,items,list}.rs`.

| # | Method | Path | File | Query / Body | Auth | Success | Errors | Rust port |
|---|---|---|---|---|---|---|---|---|
| 0 | GET | `/` | `src/index.ts:37` | — | none | `200 {status:200, message:"Welcome to Fastify"}` | — | ❌ todo |
| 1 | GET | `/stats?player=uuid` | `src/index.ts:41` | `player` uuid required | none | `200 {totalListings, activeListings, totalSold, myListings, mySold, myPurchases}` | `400 player query param is required` | ✅ `controllers/stats.rs` (JWT required; `player` query kept) |
| 2 | GET | `/items?page&pageSize&player` | `src/routes/items.ts:32` | `page` fallback 0, `pageSize` 45, `player` uuid optional | none | `200 envelope` (§11) | — | ✅ `controllers/items.rs` (public; `player` = own-listing exclusion) |
| 3 | GET | `/items/poll?page&pageSize&player&since&timeout` | `src/routes/items.ts:40` | + `since` int 0, `timeout` sec clamped 1..60 default 25 | none | `200 envelope` (busy-waits 500ms until version>since) | — | ✅ `services/items/items_poll.rs` + `version.rs` counter (bumped by add/buy) |
| 4 | GET | `/items/active?player&...` | `src/routes/items.ts:57` | `player` required | none | `200 envelope` where seller matches UUID→username | `400 player query param is required` | ✅ JWT identity, **NO `player` query** (caller-scoped; `seller_playeruuid` first, name-join fallback only for unbackfilled rows) |
| 5 | GET | `/items/bought?player&...` | `src/routes/items.ts:66` | `player` required | none | `200 envelope` from `history` where `buyer=player` | `400 ...` | ✅ JWT identity, **NO `player` query** (same-name accounts cannot leak) |
| 6 | GET | `/items/history?player&...` | `src/routes/items.ts:75` | `player` required | none | `200 envelope` from `history` where seller/buyer matches | `400 ...` | ✅ JWT identity, **NO `player` query** |
| 7 | POST | `/items/add` | `src/routes/items.ts:84` | `ItemsAddBody` legacy strings (§10.1) | none | `200 {success:true, message:"item created with id : <id>"}` + bump | `400` price len>20/name spaces/material regex/amount 1..64/blocked | ✅ JWT identity (no `playeruuid`/`playerName` body); string+numeric shapes via untagged `StrNum`; 1-round-trip `INSERT..SELECT users` |
| 8 | POST | `/list` | `src/routes/auction.ts:13` | modern numeric body (§10.1) | none | `200 {success:true, message:"item created with id : <id>"}` + bump | `400` UUID/name/material/amount/price/blocked | ✅ `controllers/list.rs` reuses the `/items/add` handler (accepts both shapes); JWT identity |
| 9 | POST | `/items/buy` | `src/routes/items.ts:114` | `{wantedId:number, buyerId:uuid}` | none | `200 {success:true, balance:<newBalance>, message:"item added with id : <historyId>"}` + bump | `400` invalid wantedId/malformed UUID, `404` not found, `400` own listing, `400` PENDING, `400` sold out, `400` insufficient funds | ✅ JWT buyer (body = `{wantedId}` only); **claim-first tx**: `PENDING` claim → own-check → atomic deduct → `SOLD` → history → seller credit, COMMIT, ROLLBACK on any failure |
| 10 | POST | `/items/retrieve` | `src/routes/items.ts:205` | `{wantedId:number, sellerId:uuid}` | none | `200 {success:true, material, amount, message:"item <id> retrieved"}` → REMOVED + bump | `400` invalid UUID, `404` not found, `400` not available, `400` not owner | ✅ done |
| 11 | GET | `/balance?player=uuid` | `src/routes/balance.ts:10` / `Rust: controllers/balance.rs:9` | `player=uuid` | **Bearer** (must match `sub`) — legacy Node still allows none but should require Bearer | `200 {playerUuid, balance}` | `400 player query param is required`, `401 missing/invalid token`, `401 you can only query your own balance` | ✅ `controllers/balance.rs` — ⚠ query param only, Bearer check **not yet enforced** |
| 12 | POST | `/balance/set` | **REMOVED** | — | — | `410 Gone` — use DB seed or internal service | — | ❌ todo (410 Gone) |
| 13 | POST | `/balance/change` | **REMOVED** | — | — | `410 Gone` — use `POST /shop/buy|sold` or `POST /items/buy` (price-from-DB, atomic) | — | ❌ todo (410 Gone) |
| 14 | GET | `/auth/status?player=uuid` | `src/routes/auth.ts` / `Rust: controllers/auth.rs:79` | `player=uuid` | none (public) | `200 {registered, hasPassword, username?}` | `400` | ✅ done |
| 15 | POST | `/auth/register` | `src/routes/auth.ts` / `Rust: controllers/auth.rs:48` | `{playerUuid, playerName, password 4..64}` | none (public) | `201 {uuid, username, message}` | `400` malformed UUID / invalid name / password / already registered | ✅ done (Argon2id) |
| 16 | POST | `/auth/login` | `src/routes/auth.ts` / `Rust: controllers/auth.rs:64` | `{playerUuid, password}` | rate-limited, public | `200 {token, token_type:"Bearer", expires_in:86400}` | `404` not registered, `401` invalid password, `429` too many attempts | ✅ done (issues JWT) |
| 17 | POST | `/auth/change` | `src/routes/auth.ts` / `Rust: controllers/auth.rs:93` | `{newPassword 4..64}` + **Bearer** (`oldPassword` in body, `uuid` from `auth.claims.sub`) | **Bearer** | `200 {success:true, message:"password changed"}` | `401 missing/invalid/expired token`, `400/404/401` | ✅ done (Bearer via `Auth` extractor) |
| 18 | GET | `/shop/catalog` | `src/routes/shop.ts:33` | — | none (public read) | `200 {version:number, lastUpdated:ISO|null, items:[{id,material,category,buy,sell}]}` | `500` unavailable | ❌ todo |
| 19 | POST | `/shop/catalog/reload` | `src/routes/shop.ts:48` | — | **Bearer (admin) or `x-plugin-secret` (Node legacy)** | `200 {success:true, message:"reloaded N items"}` — also reseeds `shop_items` | `401` unauthorized, `500` reload failed | ❌ todo |
| 20 | **★ POST** | **`/shop/buy`** | `src/routes/shop.ts:60` / `Rust: (planned)` | `{itemId?:string, material?:string, amount:number 1..320}` **no `playerUuid` — use `auth.sub`** | **Bearer** | `200 {success:true, balance:number, cost:number, amount:number, material:string, itemId:string}` | `401` missing/invalid token, `400` invalid amount / itemId required, `404` unknown item, `400` insufficient funds | ❌ todo |
| 21 | **★ POST** | **`/shop/sell`** | `src/routes/shop.ts:95` | `{itemId?, material?, amount 1..2304}` **no `playerUuid`** | **Bearer** | `200 {success:true, balance:number, gain:number, amount, material, itemId}` | `401`, `400` invalid amount / unknown item / not sellable | ❌ todo |

### 10.1 Listing create — two shapes (both must be supported)

**Legacy `POST /items/add`** (`src/routes/items.ts:84`, `src/types/items/items.ts`):
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
- `amount`/`price` are **strings**; `amount.trim()`→Number, `price.length>20` reject, material `^[A-Z0-9_]+$`, `playerName` no `\s`.

**Modern `POST /list`** (`src/routes/auction.ts:13`):
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
- Numeric `amount`/`price`; same validations.

Both normalize `source`/`origin` and enforce `isBlocked`, then `INSERT INTO items ... 'AVAILABLE' RETURNING id`.

### 10.2 Shop buy/sell — ★ NEW authoritative endpoints

**`POST /shop/buy` — buy from server shop (not AH)**

Request (plugin sends only identity + qty; **no price**):
```json
{
  "playerUuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5",
  "playerName": "Steve",
  "itemId": "diamond",
  "amount": 5
}
```
- `itemId` or `material` accepted; lookup is `getShopItemById(id)` then `getShopItemByMaterial(UPPER)`. Must accept both `id` (`"diamond"`) and `material` (`"DIAMOND"`) for backward compat.
- `amount` clamped `1..320` (`Math.floor`, finite). 320 = 5 stacks (max `BUY_STACK_COUNT` × max stack 64). Single-buy path uses 1..64; stack path uses 1..320.

Success:
```json
{ "success": true, "balance": 1250.0, "cost": 600.0, "amount": 5, "material": "DIAMOND", "itemId": "diamond" }
```
- `cost = row.buy * amount` (DB price, not client price).
- `balance` is post-deduction, from atomic `UPDATE ... RETURNING balance`.
- Also inserts `shop_purchases` audit row (best-effort; failure doesn't rollback purchase).

Failures:
| Code | Message | Condition |
|---|---|---|
| 400 | `malformed player UUID` | UUID regex fail |
| 400 | `invalid amount (1..320)` | amount out of range |
| 400 | `itemId or material required` | missing identifier |
| 404 | `unknown shop item` | not in `shop_items` |
| 400 | `insufficient funds (balance X, cost Y)` | `balance < cost` |
| 500 | `invalid item price` / `purchase failed: ...` | price NaN or DB error |

**`POST /shop/sell` — sell to server shop**

```json
{
  "playerUuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5",
  "itemId": "diamond",
  "amount": 12
}
```
- `amount` `1..2304` (full inventory). Sell price from DB `row.sell * amount`.
- Success: `{success:true, balance, gain, amount, material, itemId}` where `gain` is total credited.
- Fail `400 not sellable` if `gain<=0` (sell price 0).
- Plugin still owns inventory removal + provenance check (see §12.3) — backend **cannot verify inventory**; price is authoritative, quantity is `min(requested, actual clean count)` before calling.

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
- `envelopeData` (`src/services/items/items.ts:8`): `totalPages = ceil(total/pageSize)`, `version = getVersion()`.
- `page` 0-based (`offset=page*pageSize`); `parsePage(raw, fallback)` clamps negatives to fallback.
- `price` is `CAST(price AS REAL)` aliased to `price`; `seller` aliased to `sellerName`.

**List queries:**
- `getItems` (`items.ts:17`): `WHERE status='AVAILABLE'` + provenance filter; if `excludePlayer` adds `AND seller NOT IN (SELECT username WHERE playeruuid=$1) AND seller <> $1`.
- `getBySeller` (`48`): `WHERE status='AVAILABLE' AND (seller=$1 OR seller IN (SELECT username WHERE playeruuid=$1))`.
- `getByBuyer` (`70`): `history WHERE buyer=$1 ORDER BY id DESC`.
- `getHistory` (`89`): `history WHERE seller=$1 OR buyer=$1 OR seller IN (...)`.
- `getStats` (`112`): 6 counts in parallel.

---

## 12. Business Logic Flows

### 12.1 AH Buy (`POST /items/buy` — `src/routes/items.ts:114`)

```
validate wantedId finite & buyerId UUID
SELECT * FROM items WHERE id=$1 -> 404
if status != AVAILABLE -> 400 (PENDING => "processing", else "sold out")
ownerUuid = SELECT playeruuid FROM users WHERE username=seller
if ownerUuid == buyerId -> 400 "you cannot buy your own listing"
cost = Number(price)*amount; buyerBalance = getBalance(buyerId)
if buyerBalance < cost -> 400 "insufficient funds"

BEGIN
  UPDATE items SET status='PENDING' WHERE id=$1 AND status='AVAILABLE' RETURNING *
  if 0 rows -> ROLLBACK, 400 "sold out"
  re-check own-listing -> ROLLBACK if violated
  UPDATE items SET status='SOLD' WHERE id=$1
  INSERT INTO history (...) RETURNING id
COMMIT

changeBalance(buyerId, -cost)          // now atomic (see §9.1) — outside tx
if ownerUuid: try changeBalance(ownerUuid, +cost) catch swallow
bumpVersion()
return 200 {success:true, balance:newBalance, message:"item added with id : <historyId>"}
on error: ROLLBACK, 500
```

**Recommended hardening for port:** move balance mutations inside transaction with `SELECT ... FOR UPDATE` or single atomic updates.

### 12.2 AH Retrieve (`POST /items/retrieve` — `205`)

```
validate wantedId & sellerId UUID
SELECT * FROM items WHERE id=$1 -> 404
if status != AVAILABLE -> 400 "item is not available to retrieve"
ownerUuid = SELECT playeruuid FROM users WHERE username=seller
if !ownerUuid || ownerUuid != sellerId -> 400 "you can only retrieve your own listings"
UPDATE items SET status='REMOVED' WHERE id=$1
bumpVersion()
return 200 {success:true, material, amount, message:"item <id> retrieved"}
```

### 12.3 Shop Buy/Sell (★ NEW — `src/routes/shop.ts:60`, `ShopService.java:30`)

**Buy:**
```
plugin: freeCapacity(material) < amount -> "not enough space" (sync, no HTTP)
plugin: GET /balance?player=uuid -> balance
        if balance < estimatedCost(=local buy*amount) -> notEnoughBalance (optimistic, no buy call)
plugin: POST /shop/buy {playerUuid, playerName, itemId, amount}
backend: row = getShopItemById(itemId) ?? getShopItemByMaterial(UPPER(itemId/material))
         if !row -> 404 unknown shop item
         cost = row.buy * amount; assert finite>0
         registerIfAbsentByUuid(playerUuid, playerName)
         newBalance = changeBalance(playerUuid, -cost)  // atomic, throws insufficient funds
         INSERT INTO shop_purchases (playeruuid,itemId,material,amount,priceEach,totalCost)
         return {success:true, balance:newBalance, cost, amount, material, itemId}
backend on insufficient funds: getBalance(playerUuid) -> 400 "insufficient funds (balance X, cost Y)"
plugin on success (main thread): addItem(material, amount); handle leftover -> drop; send buySuccess(placed, cost, balance)
plugin on ApiException "insufficient funds": re-fetch balance, send notEnoughBalance
```

**Sell:**
```
plugin: countClean(material) + provenance checks (isBlocked) -> ownedClean, blockedCount
        if ownedClean==0 -> noneToSell
        toSell = min(amount, ownedClean); removed = removeClean(material, toSell) (mutates inventory)
plugin: POST /shop/sell {playerUuid, itemId, amount:removed}
backend: row = getShopItemById/Material; gain=row.sell*amount; changeBalance(uuid,+gain) -> newBalance
backend -> {success:true, balance:newBalance, gain}
plugin success: sellSuccess(removed, gain, balance)
plugin failure (ApiException/Exception): refund(remove) -> addItem back, backendUnreachable
```

**Key invariants:**
- Plugin **never** computes `delta` for money; backend computes `cost/gain` from `shop_items`.
- Sell inventory trust gap remains: backend can't verify player actually had the items. Provenance filter + `removeClean` happens plugin-side. Port should log `shop_purchases`/`shop_sales` and reconcile if needed.
- Amount caps: buy `1..320`, sell `1..2304` — enforce both sides.

---

## 13. Shop Catalog (`src/routes/shop.ts`, `src/services/shop/catalog.ts`, `src/data/items.json`)

- File `src/data/items.json`: `{ items: [{id, material, category, buy, sell}, ...] }` (~270 entries, `buy`/`sell` per-unit). Canonical file source; DB is runtime source of truth.
- **Seeding:** `seedShopItemsIfNeeded()` — runs on first shop route hit; if `shop_items` empty, bulk `INSERT OR IGNORE` from file (`src/services/shop/catalog.ts:11-38`). Idempotent.
- **Reload:** `POST /shop/catalog/reload` — `cached=null`, `reloadShopItemsFromFile()` does `DELETE + INSERT` from file, then reloads file cache, returns `reloaded N items` (`src/routes/shop.ts:48-57`). Still unauthenticated — add auth in port.
- **File cache:** `loadItems()` keeps `cached:{json,mtime}` keyed by `stat.mtimeMs`; `GET /shop/catalog` returns `{version: floor(mtime/1000), lastUpdated: ISO, ...data}`.
- **DB lookup helpers:** `getShopItemById(id)` and `getShopItemByMaterial(mat)` — both do `SELECT ... LIMIT 1` and return `ShopItemRow|null`.

**Plugin side (`ShopRepository.java:25`):** loads catalog from `GET /shop/catalog` with 72h file cache (`shop-cache.json`) + async refresh if stale >60h. Falls back to bundled `items.json`. Display prices are from cache but **purchase price is always DB price**.

---

## 14. Validation & Error Conventions

- **UUID:** `UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i` — duplicated per route.
- **Material:** `materialRe = /^[A-Z0-9_]+$/` on `material.trim()`; must be uppercase.
- **Amount:** shop buy `1..320`, shop sell `1..2304`, AH listing `1..64` — all `Number(...)` finite, `Math.floor`.
- **Price/cost/gain:** `Number(...)` finite>0; `cost=buy*amount` checked finite>0.
- **Player name:** `trim()` must not contain `\s` (`/\s/`), auth `len≥2`.
- **Password:** `4..64`.
- **Pagination:** `parsePage(raw, fallback)` — `Number(raw)` floored, ≥0 else fallback.
- **Shop itemId:** `(itemId ?? material).trim()` → lookup `id` then `UPPER(material)`; `400 itemId or material required` if empty, `404 unknown shop item` if not found.
- **Errors:** `fail(res, code, message) => res.code(code).send({success:false, message})` (`src/routes/items.ts:26`). Plugin checks `success` field or `statusCode/error` shape (`BackendClient.java:234`).
- **No global schema validation/middleware** — all inline.

---

## 15. Concurrency, Timeouts & Polling

| Concern | Current | Target advice |
|---|---|---|
| Long-poll `/items/poll` | Busy-wait `sleep(500)` until `version>since` or deadline `clamped 1..60s default 25` (`src/routes/items.ts:40`) | Replace with notify/pubsub or DB `LISTEN/NOTIFY` |
| Global version | In-memory counter — reset on restart, single-instance | `meta(version)` row with `UPDATE meta SET version=version+1 RETURNING` |
| AH buy race | `UPDATE ... WHERE status='AVAILABLE' RETURNING` is linearization point | Keep; add `SELECT FOR UPDATE` on Postgres |
| **Balance race** | **Fixed 2026-08-30:** `UPDATE ... SET balance=balance+$1 WHERE balance+$1>=0 RETURNING balance` (`src/services/balance/balance.ts:35`) | Port must use atomic check; `0 rows → insufficient funds` |
| SQLite locking | Single writer; `busy_timeout` not set | Set `PRAGMA busy_timeout=5000` |
| Shop concurrent buys | Same atomic update prevents overspend; second buyer gets `insufficient funds` | No extra locking needed |

---

## 16. Security Notes

- **Zero-trust shop (new):** `POST /shop/buy|sold` look up price from `shop_items`; plugin delta is **never trusted**. Old path via `POST /balance/change` with client `delta` is deprecated for shop. Enforce by removing `delta` acceptance for shop or requiring `itemId`.
- **No auth guard** on economy/shop endpoints in legacy Node — **new MC-only backend must require `Authorization: Bearer <JWT>`** on all mutating MC endpoints (`/shop/buy|sold`, `/list`, `/items/*`). Node legacy is deprecated.
- Rate limiting only on `POST /auth/login` (in-memory per `uuid:ip`). Add rate limits to `/shop/buy|sold` if needed.
- CORS not configured — add if web frontend needs it.
- `shop_purchases` audit table — log all buys; add `shop_sales` if you want sell audit.

---

## 17. Plugin Integration (Paper/Spigot `untitled/`)

| File | Role (after refactor) |
|---|---|
| `src/main/java/com/revokers/bazzar/api/BackendClient.java:51` | `getBalance(UUID)` — `GET /balance?player=` |
| `BackendClient.java:58` | `changeBalance` — still used only for AH seller credit fallback; **deprecated for shop** |
| `BackendClient.java:183` | `shopBuy(UUID,String,String,int) -> ShopResult(balance,costOrGain,amount,material,itemId)` — `POST /shop/buy` |
| `BackendClient.java:198` | `shopSell(...)` — `POST /shop/sell` |
| `shop/service/ShopService.java:30` | `buy()` — `GET /balance` pre-check → `shopBuy` → `addItem` on main thread |
| `shop/service/ShopService.java:83` | `sell()` — `removeClean` → `shopSell` → `sellSuccess` or `refund` |
| `shop/model/ShopRepository.java:25` | Loads `GET /shop/catalog` with 72h `shop-cache.json`; display only |
| `shop/gui/ItemDetailGUI.java:17` | Detail GUI, shows `buy/sell` from `ShopItem`; buttons call `ShopService` |
| `shop/listener/ShopListener.java:102` | Routes GUI clicks to `ShopService.buy/sell/buyStack` |
| `util/Messages.java` | `notEnoughBalance(balance,cost)`, `buySuccess(placed,name,cost,balance)`, `sellSuccess`, `backendUnreachable` |
| `Bazzar.java` | Plugin bootstrap, registers `ShopService` + `BackendClient(baseUrl)` |

**Client timeout:** 30s (`BackendClient.java:38`). Handle `3xx` redirect as `ApiException` with hint to use `https://`.

---

## 18. Tests as Contract (port these first)

**`test/api.test.ts` (AH):**
1. `POST /list {DIAMOND, amount 1, price 100}` → `success:true`
2. `POST /list {material:"diamond"}` → `success:false` (lowercase reject)
3. `GET /items?page=0&pageSize=45` → `page==0, items[0].sellerName=="Steve", price==100`
4. `GET /items/active?player=<uuid>` → `total>0`
5. `POST /items/buy` own listing → `400 you cannot buy your own listing`
6. Fund `/balance/change delta:10000` → buy as other UUID → `200 success:true`; then `GET /items/bought` and `/items/history` → `total>0`
7. `POST /items/buy wantedId:999999` → `404`

**New shop contracts (add these):**
8. Seed balance via DB `INSERT INTO users ... balance=1000` → `POST /shop/buy {itemId:"stone", amount:1}` cost `3` → `{success:true, balance:997, cost:3}` → `GET /balance` → `997`
9. `POST /shop/buy {itemId:"stone", amount:1000}` with balance 10 → `400 insufficient funds (balance 10, cost ...)` and balance unchanged
10. `POST /shop/buy {itemId:"UNKNOWN_ITEM", amount:1}` → `404 unknown shop item`
11. `POST /shop/buy {itemId:"stone", amount:500}` → `400 invalid amount (1..320)`
12. Crafted price attack: `POST /shop/buy {itemId:"stone", amount:1}` with concurrent manual `UPDATE shop_items SET buy=9999` → cost reflects DB value `9999`, not client value
13. `POST /shop/sell {itemId:"stone", amount:5}` → `{success:true, gain:5, balance:+5}`
14. `POST /shop/catalog/reload` → `{success:true, message:"reloaded N items"}` and subsequent buy reflects new price

Use as **contract tests** in target language before other work.

---

## 19. Deployment & Ops

- **CI** `.github/workflows/ci.yml`: `bun install` + `bun test` on PRs; on push to `master` SCP `package.json bun.lock .env.example src test deploy` to host and `bash deploy/remote.sh`.
- **Service** `deploy/bazzar.service`: `WorkingDirectory=/var/www/bazzar`, `EnvironmentFile=/var/www/bazzar/.env`, `ExecStart=/root/.bun/bin/bun run src/index.ts`, `Restart=on-failure`.
- **Rust port deploy:** `Cargo.toml` (`bazzar-back` 0.1.0, Actix, SeaORM Postgres, Argon2, jsonwebtoken). Binary `src/main.rs`, `migration/` for SeaORM migrations. Env is `.env` with Postgres `DATABASE_URL`.
- **Health:** `GET /` + `GET /stats?player=uuid` probes.
- **Shop seed on deploy:** ensure `src/data/items.json` is present on host; on first start `shop_items` will auto-seed. If catalog changes, `POST /shop/catalog/reload` or restart.

---

## 20. What to Implement in the Target Language (Checklist)

### MVP — MC-ONLY (drop-in replacement — updated 2026-08-30, UI stripped)

- [ ] Config from `DATABASE_URL`, `PORT` (fallback 3000), `HOST`, `TRUST_PROXY`, keep `WEBSITE_URL` compat.
- [ ] SQLite open + DDL (including `shop_items`, `shop_purchases`) + migration check + `WAL` + `foreign_keys=ON` + pool shim or native queries.
- [ ] In-memory `version` + `bumpVersion`/`getVersion` + envelope `{page,pageSize,total,totalPages,version,items}`.
- [ ] All **22 endpoints** exactly as §10 with same status codes/messages/keys (`sellerName`, `you cannot buy your own listing`, `insufficient funds`, etc.) — includes new `POST /shop/buy|sold`.
- [ ] Both listing shapes (`/items/add` string + `/list` numeric) + `registerIfAbsent` upsert.
- [ ] Provenance `normalize*` + `isBlocked` + SQL filters.
- [ ] AH buy tx: `PENDING` guard + `SOLD` + `history` insert (+ atomic balance ops).
- [ ] Balance: `get/set/change` with **atomic** `UPDATE ... WHERE balance+delta>=0 RETURNING` + `deductBalanceAtomic`.
- [ ] Shop: `shop_items` seed from `items.json` + `getShopItemById/Material` + `POST /shop/buy|sold` with amount caps (`1..320` buy, `1..2304` sell) + `shop_purchases` audit + `POST /shop/catalog/reload` reseed.
- [ ] Auth: `status/register/login/change` with Argon2id + `5/60s` rate limit — no JWT required for MVP.
- [ ] Shop catalog: `items.json` mtime-cache + `GET /shop/catalog` + `POST /shop/catalog/reload`.
- [ ] `GET /` welcome + `GET /stats` counts.
- [ ] `GET /items/poll` long-poll (clamped 1..60s, 500ms wake).
- [ ] Pagination 0-based, `totalPages=ceil(total/pageSize)`.
- [ ] Validation regexes and error envelope `{success:false, message:String}`.

### Hardening (recommended post-MVP)

- [ ] Replace in-memory version with durable counter; replace sleep-poll with notify/pubsub.
- [ ] Add real auth (JWT) + protect admin endpoints (`/balance/set`, `/shop/catalog/reload`, `/shop/buy|sold` if needed) — Rust port has `jsonwebtoken` ready.
- [ ] Add `PRAGMA busy_timeout`, connection pool, request schema validation, global error handler (`app.setErrorHandler` already normalizes to `{success:false,message}`).
- [ ] Switch `items.seller` to UUID (backfill) to eliminate name↔uuid join.
- [ ] Normalize `price` column to `REAL` and add `shop_sales` audit table.
- [ ] Rate-limit `POST /shop/buy|sold` per player/IP.
- [ ] Add `GET /shop/purchases?player=uuid` for audit UI.

---

## 21. Key Files to Read When Porting (updated)

```
src/db.ts:1-142                 DDL + migration + pool shim + version counter (now includes shop_items/shop_purchases)
src/index.ts:1-65               Route registration, error handler, security/rateLimit/banGuard, GET / + /stats
src/routes/items.ts:1-239       AH feeds + buy + retrieve + provenance
src/routes/auction.ts:1-47      Modern listing create (numeric)
src/routes/balance.ts:1-40      Balance CRUD (now uses atomic changeBalance)
src/routes/auth.ts              Auth + rate limit
src/routes/shop.ts:1-116        ★ Catalog cache + BUY/SELL authoritative endpoints
src/services/items/items.ts     Envelope + SQL for list/history/stats
src/services/balance/balance.ts:1-60  ★ Atomic changeBalance + deductBalanceAtomic
src/services/shop/catalog.ts:1-69     ★ Seed + lookup + reload helpers
src/services/auth/auth.ts       User lookups
src/utils/provenance.ts         Block logic (must replicate exactly)
src/utils/registerIfAbsent.ts   Upsert helper
src/types/items/items.ts        Legacy body type
src/data/items.json             Shop catalog shape (~270 entries)
test/api.test.ts                Contract tests — port first
test/ui.test.ts                 UI/ban contract tests
api.md                          Human-readable API contract (update to include POST /shop/buy|sold)
untitled/src/main/java/com/revokers/bazzar/api/BackendClient.java  ★ shopBuy/shopSell + ShopResult
untitled/src/main/java/com/revokers/bazzar/shop/service/ShopService.java  ★ zero-trust buy/sell flow
```

---

> Tip: Start the port by copying `test/api.test.ts` into the target language's test suite and making it pass before adding new features. Then add the 7 new shop contract tests from §18 and verify price tampering is impossible (client price ignored, DB price wins).
