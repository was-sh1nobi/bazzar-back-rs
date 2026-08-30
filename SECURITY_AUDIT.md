# Bazzar Security Audit — All Exploits & Fixes (2026-08-30)

> Basis: untrusted Paper plugin == attacker can run arbitrary `curl` with any `playerUuid`. All `req.body`/`req.query` fields forgeable. Backend is source of truth.

> **2026-08-31 Update — Per-User JWT Bearer Model (New Backend):**
> New Rust backend (`RustroverProjects/bazzar-back`) issues a per-user HS256 JWT on `POST /auth/login` (`auth.rs:98` `create_jwt(sub, username)` TTL 24h `auth.rs:16`, `jti` `uuid::Uuid::new_v4()` `auth.rs:81`). Plugin holds the token **in server memory** `Map<UUID, token>` (never on disk) and sends `Authorization: Bearer <jwt>` on **every** mutating request (`Auth` extractor `auth.rs:180` verifies signature, `exp`+leeway 60s, `iss=bazzar-back`, `aud=bazzar-client`, pins `HS256` reject `none`). Node backend still uses shared `PLUGIN_SECRET` (`pluginAuth.ts`); the two models are compared below — JWT per-user is the recommended canonical.

---

## Summary Table

| # | Severity | Title | Backend File:Line | Plugin File:Line | Exploit | Fix (Backend / Plugin) |
|---|----------|-------|-------------------|------------------|---------|------------------------|
| E01 | CRITICAL → **MITIGATED by JWT** | **UUID Spoofing — No Auth** | `Rust: auth.rs:180` `controllers/balance.rs:9` `Node: shop.ts:60`, `auction.ts:18`, `items.ts:114` | `BackendClient.java:51,185,202` + `AuthService.java:55` | Before: `curl POST /shop/buy {"playerUuid":"<victim>"}` drains victim. After: every mutating endpoint **must** extract `Auth` (`Authorization: Bearer <jwt>`) and use `auth.claims.sub` as canonical UUID — `playerUuid` in body is **ignored**. Without valid JWT → `401 missing/invalid/expired token` (`auth.rs:197,232`). | **BE (Rust canonical):** `POST /auth/login` issues `create_jwt(sub, username)` (`services/auth.rs:173`); `src/auth.rs:180` `Auth::from_request` verifies HS256, `exp`+60s leeway, `iss`/`aud`, `sub` required. All `POST /shop/buy|sold`, `/list`, `/items/add|buy|retrieve`, `GET /balance` must take `auth: Auth` extractor — **delete** `playerUuid` from body and use `auth.player_uuid()`. **BE (Node legacy):** keep `pluginAuth.ts` shared secret as fallback until JWT ported. **PL:** On `AuthService.login` success store `token` in memory `Map<UUID,String>` (never file), attach `Authorization: Bearer <token>` in `BackendClient` on every request; refresh on `401 token expired` by re-login. |
| E02 | CRITICAL | **Shop Sell Infinite Money — No Inventory Check** | `shop.ts:95` | `ShopService.java:133` | `for i in {1..1000}; do curl /shop/sell {"playerUuid":"own","material":"COBBLESTONE","amount":2304}` mints `gain=sell*amt` each time | **BE:** Rate-limit `sell` per UUID `20/min` + gain cap `50000` + log, added in `shop.ts:84`. True fix: require signed inventory receipt (future: `removeClean` returns HMAC token backend verifies). Doc as known limitation. **PL:** `ShopService.java:133` cooldown `500ms` per player + remove items *before* backend call (already) and refund on fail. |
| E03 | CRITICAL | **Balance Writes Public (`POST /balance/set|change`)** | `balance.ts:17,27` (removed) | `BackendClient.java:57` | `curl POST /balance/change {"playerUuid":"victim","delta":1000000}` or `{"delta":-9999}` on victim | **BE:** Deleted both POST handlers — only `GET /balance` remains `balance.ts:9`. All mutations private: `shop.ts:60,95` and `items.ts:114` via `services/balance/balance.ts:32` atomic. **PL:** `BackendClient.java:57` now `@Deprecated` throwing. |
| E04 | CRITICAL | **Provenance Laundering — `source`/`origin` Client-Controlled** | `auction.ts:26`, `items.ts:93`, `provenance.ts:19` | `BackendClient.java:99`, `AuctionService.java:284` | `curl /list {"source":"player","origin":"survival"}` for creative-spawned items bypasses `isBlocked` | **BE:** Keep `isBlocked` but ignore client strings for shop (shop never uses them); for AH keep double-check + defense-in-depth SQL filter `services/items/items.ts:21`. **PL:** Always send real `ProvenanceService.getOrigin(item)` values; do not allow caller to override. |
| E05 | HIGH | **Arbitrary Material Injection** | `items.ts:89`, `auction.ts:20` `materialRe=/^[A-Z0-9_]+$/` | `AuctionService.java:215` | `BARRIER`, `COMMAND_BLOCK` listed | **BE:** Keep regex + add length cap `<=64` + optional whitelist against `shop_items` or `Material.values()`. **PL:** `Material.matchMaterial` already validates; reject unknown. |
| E06 | HIGH | **Price Type Confusion (`items/add` TEXT)** | `items.ts:87` `price.length>20` only | — | `price="NaN"` → `cost=NaN` bypasses `balance < cost` | **BE:** Added `priceNum = Number(price.trim()); if !finite||<=0 fail 400` `items.ts:88`. Store `String(priceNum)`. |
| E07 | HIGH | **Seller Identity as Username String (Hijack)** | `items.ts:138` `seller TEXT` | `Bazzar.java:45` offline UUID | Attacker renames victim via `registerIfAbsent` | **BE:** `isBanned` still username-based; future: migrate `seller` to `seller_uuid` FK. For now ensure `registerIfAbsent` is `await`ed and username update is `ON CONFLICT playeruuid DO UPDATE`. **PL:** Use online-mode `online-mode=true`. |
| E08 | HIGH | **AH Buy Race — Non-Atomic Balance** | `items.ts:146` balance check outside tx, credit after COMMIT | — | Double-spend via concurrent `POST /items/buy` | **BE:** `changeBalance` already atomic `balance.ts:32`; buy flow still checks outside tx but linearization via `UPDATE status=PENDING WHERE AVAILABLE` + atomic deduct limits window. Future: move deduct inside tx. |
| E09 | MEDIUM | **Unauthenticated `POST /shop/catalog/reload` DoS** | `shop.ts:48` | — | Loop `DELETE+INSERT` → lock contention | **BE:** Now guarded by `pluginAuth` (requires `x-plugin-secret`) + implicitly rate-limited via global `mcMax=120/min`. |
| E10 | MEDIUM | **BanGuard Bypass — Missing `playeruuid` Key** | `ban.ts:42` `extractPlayerUuids` | — | Banned can `POST /items/add {"playeruuid":"<banned>"}` | **BE:** Fixed `ban.ts:48` to push `body.playeruuid` (lowercase) + `player`. |
| E11 | MEDIUM | **Pagination DoS (`pageSize=1e9`)** | `items.ts:32` `parsePage` no cap | — | `SELECT LIMIT 1e9` OOM | **BE:** Added `clampPageSize = min(100, max(1, parsePage))` `items.ts:33`. |
| E12 | MEDIUM | **Missing `await` on `registerIfAbsent`** | `shop.ts:72`, `auction.ts:33`, `items.ts:86` | — | Race username clobber | **BE:** Fixed `items.ts:86` + `shop.ts:72,92` to `await`. |
| E13 | HIGH | **Auth Without Ownership Proof** | `auth.ts:36` `ensureUser` | `AuthService.java:28` offline | Pre-register victim UUID | **BE/PL:** Require `online-mode=true` or Mojang session verify; otherwise UUID is bearer token (inherent to offline mode). Doc as deployment requirement. |
| E14 | MEDIUM | **PlayerName XSS / Injection** | `items.ts:88` only `\s` check | `Messages.java:153` `escape` | `<script>` stored in `seller` | **BE:** Keep `escape` in UI + add length cap `3..16` + `USERNAME_RE`. **PL:** Same. |
| E15 | MEDIUM | **Shop Inventory Race / Cooldown Missing**
| E16 | HIGH | **JWT Theft / No Revocation / In-Memory Only** | `Rust: auth.rs:81` `auth.rs:20` `main.rs:13` (no DB) | `AuthService.java:55` `Bazzar.java:32` | Token stored only in `JWT_SECRET` memory + plugin memory. No revocation list: stolen token valid until `exp` (24h). `JWT_SECRET` fallback `dev-only-please-change...` in prod leaks. No `jti` storage, replay allowed. `GET /balance` still public without `Auth` leaks existence. | **BE:** Persist `jti` in `users` or `revoked_tokens` with `exp`, check on verify; add `POST /auth/logout` revokes; enforce `JWT_SECRET len>=32` fail-closed in prod (`auth.rs:20`). Require `Auth` on `GET /balance` too (or at least `GET /balance?player=` must match `auth.sub`). **PL:** Store token only in `ConcurrentHashMap`, clear on `PlayerQuitEvent` and `500` logout; never log header; use `https` only. | | `shop.ts:64` no per-UUID limit | `ShopService.java:30` no cooldown | Spam buy/sell 1000/sec | **BE:** Per-UUID buckets `buy 30/min sell 20/min` `shop.ts:68`. **PL:** `ShopService.java:25` `lastBuy/lastSell` 500ms cooldown + `Messages.cooldown()`. |

---

## Detailed Fix per File

### Backend `bazzar-back`

1. **New `src/plugins/pluginAuth.ts`** — `requirePluginSecret` checks `PLUGIN_SECRET` env; `registerPluginAuth` hooks `preValidation` for `POST /shop/buy|sold`, `/list`, `/items/add|buy|retrieve`, `/shop/catalog/reload`. In prod without secret → 503 closed.
2. **`src/index.ts:12,31`** — import + register `registerPluginAuth` after rateLimit.
3. **`.env.example`** — added `PLUGIN_SECRET` description.
4. **`src/services/ban/ban.ts:48`** — include `body.playeruuid`.
5. **`src/routes/items.ts:33,86`** — `clampPageSize`, numeric price validation, `await registerIfAbsent`.
6. **`src/routes/shop.ts:64,84`** — `await registerIfAbsentByUuid`, per-UUID shopBuckets, sell gain cap.
7. **`src/routes/balance.ts:9`** — removed `POST /set|/change`, left `GET` only with comment.
8. **`test/api.test.ts:73`** — fund via direct `pool.query INSERT` not HTTP.

### Plugin `untitled`

1. **`src/main/resources/config.yml`** — add `PLUGIN_SECRET` via `.env` (already via `Env.java`).
2. **`src/main/java/com/revokers/bazzar/Bazzar.java:38`** — load `PLUGIN_SECRET`/`API_SECRET` from `Env` and pass to `BackendClient`.
3. **`src/main/java/com/revokers/bazzar/api/BackendClient.java:48,228`** — store `pluginSecret`, send `x-plugin-secret` on every `POST`; `changeBalance` now deprecated throwing.
4. **`src/main/java/com/revokers/bazzar/shop/service/ShopService.java:25`** — `lastBuy/lastSell` maps + 500ms cooldown, `checkCooldown` before buy/sell, uses `Messages.cooldown()`.
5. **`src/main/java/com/revokers/bazzar/util/Messages.java:148`** — added `cooldown()`.

---

## Remaining Risk — Shop Sell Inventory Attestation + JWT Lifecycle

Backend still cannot cryptographically prove player owned `amount` before `POST /shop/sell`. Mitigations above (rate limit, gain cap, plugin cooldown) make farming linear-time but not zero. Full fix requires:

- Plugin after `removeClean` generates `receipt = HMAC(JWT_SECRET or derived, playerUuid + material + amount + timestamp)` and backend verifies (redundant with JWT but adds inventory proof).
- With JWT, the receipt can be the JWT itself bound to `jti` — still need inventory proof separately.
- Or backend tracks `shop_purchases` + `shop_sell` ledger and enforces `netSell <= netBuy + mined` heuristic.
- Until then treat `shop/sell` as semi-trusted and monitor `shop_purchases` logs.

---

## Deployment Checklist

- [ ] **New JWT model:** Set `JWT_SECRET` (32+ random, `openssl rand -hex 32`) in `bazzar-back/.env` (Rust) and ensure `PLUGIN_SECRET` is removed or kept only as fallback for Node. Verify `curl without Authorization: Bearer → 401`.
- [ ] (Node legacy) Set `PLUGIN_SECRET` (32+ hex) in `bazzar-back/.env` and `plugins/bazzar/.env` if still on Node backend.
- [ ] Set `online-mode=true` in `server.properties` (or Bungee online_mode) to prevent offline UUID spoof.
- [ ] Restart backend and plugin; verify `curl without x-plugin-secret → 401` and `with → 200`.
- [ ] Run `POST /shop/buy` and `/shop/sell` flows; verify balance via `GET /balance`.
