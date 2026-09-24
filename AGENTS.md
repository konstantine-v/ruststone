# Agent guide for Ruststone

Ruststone is a **DB-less Loco 1.1** app (`loco-rs` with the `cli` and `cache_inmem` features only). It scrapes the FFXIV Lodestone and serves typed JSON. It has no database, migrations, auth, mailers, workers or views, so Loco generators for those don't apply. Prefer Loco built-ins (config, middleware, `ctx.cache`, `ctx.shared_store`) over new crates.

## Layout

```
src/app.rs                 # Hooks: after_context builds the Lodestone client; routes
src/settings.rs            # typed `settings:` block (deny_unknown_fields)
src/controllers/           # thin handlers: extract → lodestone.fetch → respond
src/lodestone/
  mod.rs                   # Lodestone client, Page trait, PageKind, Cached<P>
  selector.rs              # Sel (compiled CSS + regex), ListSel, PagedSel
  selectors.rs             # Selectors: every vendored file, loaded at boot
  ids.rs                   # validated IDs / query values (400 on bad input)
  error.rs                 # LodestoneError → HTTP status
  pages/                   # one module per Lodestone page (output types + parsing)
assets/selectors/          # vendored lodestone-css-selectors (byte-for-byte)
tests/requests/            # request tests against a local fixture Lodestone (:5199)
tests/live/                # #[ignore] drift checks against the real Lodestone
```

## How a request flows

1. The handler extractors validate input: `Path<CharacterId>`, `Query<SearchQuery>`, and so on.
2. `Lodestone::fetch::<P>(&ctx.cache, req)` checks the cache. The key is `lodestone:{kind}:{region}:{json(req)}`.
3. On a miss it acquires a semaphore permit and makes a GET with `reqwest`.
4. The upstream status is mapped to a `LodestoneError`.
5. The HTML is parsed on `spawn_blocking` with `P::parse`, then serialized once to a `RawValue`.
6. The JSON is cached with `ttl(P::KIND)`.
7. The handler returns `Cached<P>` with `Cache-Control`. Cache hits never deserialize.

## Adding a Lodestone page

1. Vendor the selector file into `assets/selectors/` and update `UPSTREAM`.
2. In `src/lodestone/pages/<name>.rs`:
   - Write a `#[derive(Deserialize)] #[serde(rename_all = "SCREAMING_SNAKE_CASE")]` selector struct whose fields are `Sel`, `Option<Sel>`, `ListSel<E>`, `PagedSel<E>`, or nested groups.
   - Write a `#[derive(Serialize)]` output struct.
   - `impl Page`: `KIND`, `Request` (validated ID types), `url` (use `page_url`, which percent-encodes the path) and `parse`.
3. Add a field to `Selectors` and load it with `file!("…")`. A missing key or an invalid selector or regex fails the `vendored_selectors_load` test and boot.
4. If the page needs its own TTL, add a `PageKind` variant and map it in `CacheTtl::for_kind`.
5. Add a handler that calls `respond::<P>(…)`, register its `Routes` in `src/app.rs`, and add a request test plus a fixture in `tests/requests/fixtures.rs`.

Conventions:
- Output enums use `#[serde(rename_all(serialize = "snake_case", deserialize = "UPPERCASE"))]`, so the same enum can key a selector file (`DARKKNIGHT`) and serialize idiomatically (`dark_knight`).
- Output types are `Serialize` only. The cache stores JSON and never reads it back into Rust types.

## Commands

```
cargo loco start
cargo loco routes
cargo test
cargo test -- --ignored   # live Lodestone
cargo clippy --all-features --all-targets -- -D warnings -W clippy::pedantic -W clippy::nursery -W rust-2018-idioms
```

- Loco docs: https://loco.rs/docs
- Framework agent guide: https://loco.rs/AGENTS.md
