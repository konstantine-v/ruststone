# Ruststone

A fast, typed [Lodestone](https://na.finalfantasyxiv.com/lodestone/) parser and JSON API for Final Fantasy XIV, built with [Loco](https://loco.rs). It is a Rust rebuild of [xivapi/nodestone](https://github.com/xivapi/nodestone) and is driven by the same [lodestone-css-selectors](https://github.com/xivapi/lodestone-css-selectors) files.

- **Typed end to end.** IDs, query values and responses are Rust types. Bad input gets a `400` before any upstream request is made.
- **Fast.** It uses one pooled HTTP client, parses HTML off the async runtime, caps concurrent upstream requests, and caches serialized JSON, so a cache hit skips parsing entirely.
- **Small.** There is no database, auth, mailer or job queue.

## Running

```sh
cargo loco start                      # http://localhost:5150
LOCO_ENV=production cargo loco start  # JSON logs, binds 0.0.0.0
```

Set `LODESTONE_REGION` to `na`, `eu`, `fr`, `de` or `jp` to change the regional site. Also available: `PORT`, `LOG_LEVEL`, `CACHE_MAX_ENTRIES`, `LODESTONE_TIMEOUT_MS` and `LODESTONE_MAX_CONCURRENT`. Everything else lives under `settings:` in `config/*.yaml` (see `src/settings.rs`). Unknown keys fail at boot.

## API

All routes are `GET`. Responses are JSON with `Cache-Control: public, max-age=<cache TTL>`.

| Route | Query | Returns |
|---|---|---|
| `/api/characters` | `name`, `world` \| `dc`, `page` | character search |
| `/api/characters/{id}` | `include=class_jobs,achievements,mounts,minions` | `{ character, …included }` |
| `/api/characters/{id}/class_jobs` | | class/job levels, Bozja, Eureka |
| `/api/characters/{id}/achievements` | `page` | achievements, points, pagination |
| `/api/characters/{id}/mounts` | | mounts |
| `/api/characters/{id}/minions` | | minions |
| `/api/free_companies` | `name`, `world` \| `dc`, `page` | free company search |
| `/api/free_companies/{id}` | `include=members` | `{ free_company, members? }` |
| `/api/free_companies/{id}/members` | `page` | members |
| `/api/linkshells` · `/api/linkshells/{id}` | search / `page` | linkshell and its members |
| `/api/cwls` · `/api/cwls/{id}` | `name`, `dc` / `page` | cross-world linkshell and its members |
| `/api/pvp_teams` · `/api/pvp_teams/{id}` | `name`, `dc` | PvP team and its members |

Sections added with `include=` are fetched concurrently with the main page. A section you didn't request is left out of the response. A section the Lodestone reports as private is `null`.

The API uses these status codes:

| Status | Meaning |
|---|---|
| 400 | invalid input |
| 404 | not on the Lodestone |
| 403 | private |
| 429 | the Lodestone is rate limiting |
| 503 | Lodestone maintenance |
| 504 | the Lodestone timed out |
| 502 | other upstream failure |

Error bodies look like `{"error": "not_found", "description": "…"}`.

## MCP

`POST /mcp` is a [Model Context Protocol](https://modelcontextprotocol.io) server (stateless streamable HTTP) that exposes the API as read-only tools: `search_characters`, `get_character`, `get_character_class_jobs`, `get_character_achievements`, `get_character_mounts`, `get_character_minions`, `search_free_companies`, `get_free_company`, `get_free_company_members`, `search_linkshells`, `get_linkshell`, `search_cwls`, `get_cwls`, `search_pvp_teams` and `get_pvp_team`. IDs are passed as strings.

Tools call the `/api` router in-process, so validation, caching and error handling match the REST API. Add a route and a tool entry in `src/mcp.rs` together.

```sh
claude mcp add --transport http ruststone http://localhost:5150/mcp
```

The endpoint has no auth. Put it behind your own proxy before exposing it publicly.

## Development

```sh
cargo test                       # unit tests + request tests against a local fixture server
cargo test -- --ignored          # live checks against the real Lodestone (selector drift)
cargo loco routes
```

The selector files are vendored in `assets/selectors/`; `UPSTREAM` records the source commit. See [AGENTS.md](AGENTS.md) for how the code fits together and how to add a page.
