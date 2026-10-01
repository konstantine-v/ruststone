//! [Model Context Protocol](https://modelcontextprotocol.io) endpoint.
//!
//! `POST /mcp` speaks the stateless streamable-HTTP transport (JSON-RPC in,
//! one JSON response out). Each tool is a thin description of one `/api`
//! route and is executed by calling the application router in-process, so
//! validation, caching and errors are exactly those of the REST API and the
//! two cannot drift apart.

use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{header, Method, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router as AxumRouter,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tower::ServiceExt;
use url::form_urlencoded;

const LATEST_PROTOCOL: &str = "2025-06-18";
const SUPPORTED_PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
/// Upper bound on a tool result read back from the router.
const MAX_RESULT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy)]
enum Kind {
    /// Lodestone ID, sent as a string (free company IDs exceed 2^53).
    Id,
    Text,
    Page,
    /// Comma-joined `include=` sections.
    Include(&'static [&'static str]),
}

struct Param {
    name: &'static str,
    kind: Kind,
    required: bool,
    /// Substituted into `{name}` in the path instead of the query string.
    in_path: bool,
    description: &'static str,
}

struct Tool {
    name: &'static str,
    description: &'static str,
    path: &'static str,
    params: &'static [Param],
}

const fn id(description: &'static str) -> Param {
    Param {
        name: "id",
        kind: Kind::Id,
        required: true,
        in_path: true,
        description,
    }
}

const fn page(description: &'static str) -> Param {
    Param {
        name: "page",
        kind: Kind::Page,
        required: false,
        in_path: false,
        description,
    }
}

const fn include(sections: &'static [&'static str], description: &'static str) -> Param {
    Param {
        name: "include",
        kind: Kind::Include(sections),
        required: false,
        in_path: false,
        description,
    }
}

const fn text(name: &'static str, required: bool, description: &'static str) -> Param {
    Param {
        name,
        kind: Kind::Text,
        required,
        in_path: false,
        description,
    }
}

const NAME: Param = text("name", true, "Name to search for (1-64 characters).");
const WORLD: Param = text(
    "world",
    false,
    "Restrict to one world, e.g. `Gilgamesh`. Wins over `dc`.",
);
const DC: Param = text("dc", false, "Restrict to one data center, e.g. `Aether`.");
const DC_ONLY: Param = text(
    "dc",
    false,
    "Restrict to one data center, e.g. `Aether`. (Worlds are not searchable here.)",
);
const SEARCH_PAGE: Param = page("1-based result page (default 1).");
const MEMBER_PAGE: Param = page("1-based page of the member list (default 1).");

const TOOLS: &[Tool] = &[
    Tool {
        name: "search_characters",
        description: "Search FFXIV characters by name. Returns matches with their Lodestone IDs.",
        path: "/api/characters",
        params: &[NAME, WORLD, DC, SEARCH_PAGE],
    },
    Tool {
        name: "get_character",
        description: "Get a character's profile by Lodestone ID. Use `include` to fetch class/job \
                      levels, achievements (first page), mounts and minions in the same call.",
        path: "/api/characters/{id}",
        params: &[
            id("Lodestone character ID."),
            include(
                &["class_jobs", "achievements", "mounts", "minions"],
                "Extra sections to fetch. A section the player keeps private comes back `null`.",
            ),
        ],
    },
    Tool {
        name: "get_character_class_jobs",
        description: "Get a character's class and job levels, including Bozja and Eureka.",
        path: "/api/characters/{id}/class_jobs",
        params: &[id("Lodestone character ID.")],
    },
    Tool {
        name: "get_character_achievements",
        description: "Get a page of a character's achievements, with total points and pagination.",
        path: "/api/characters/{id}/achievements",
        params: &[
            id("Lodestone character ID."),
            page("1-based achievements page (default 1)."),
        ],
    },
    Tool {
        name: "get_character_mounts",
        description: "Get the mounts a character owns.",
        path: "/api/characters/{id}/mounts",
        params: &[id("Lodestone character ID.")],
    },
    Tool {
        name: "get_character_minions",
        description: "Get the minions a character owns.",
        path: "/api/characters/{id}/minions",
        params: &[id("Lodestone character ID.")],
    },
    Tool {
        name: "search_free_companies",
        description: "Search free companies by name. Returns matches with their Lodestone IDs.",
        path: "/api/free_companies",
        params: &[NAME, WORLD, DC, SEARCH_PAGE],
    },
    Tool {
        name: "get_free_company",
        description: "Get a free company by Lodestone ID. `include=members` adds the first page \
                      of members.",
        path: "/api/free_companies/{id}",
        params: &[
            id("Lodestone free company ID (a large integer, pass it as a string)."),
            include(&["members"], "Extra sections to fetch."),
        ],
    },
    Tool {
        name: "get_free_company_members",
        description: "Get a page of a free company's members.",
        path: "/api/free_companies/{id}/members",
        params: &[id("Lodestone free company ID."), MEMBER_PAGE],
    },
    Tool {
        name: "search_linkshells",
        description: "Search (world) linkshells by name. Returns matches with their IDs.",
        path: "/api/linkshells",
        params: &[NAME, WORLD, DC, SEARCH_PAGE],
    },
    Tool {
        name: "get_linkshell",
        description: "Get a linkshell and a page of its members.",
        path: "/api/linkshells/{id}",
        params: &[id("Lodestone linkshell ID."), MEMBER_PAGE],
    },
    Tool {
        name: "search_cwls",
        description: "Search cross-world linkshells by name. Returns matches with their IDs.",
        path: "/api/cwls",
        params: &[NAME, DC_ONLY, SEARCH_PAGE],
    },
    Tool {
        name: "get_cwls",
        description: "Get a cross-world linkshell and a page of its members.",
        path: "/api/cwls/{id}",
        params: &[
            id("Cross-world linkshell ID (40 hexadecimal characters)."),
            MEMBER_PAGE,
        ],
    },
    Tool {
        name: "search_pvp_teams",
        description: "Search PvP teams by name. Returns matches with their IDs.",
        path: "/api/pvp_teams",
        params: &[NAME, DC_ONLY, SEARCH_PAGE],
    },
    Tool {
        name: "get_pvp_team",
        description: "Get a PvP team and its members.",
        path: "/api/pvp_teams/{id}",
        params: &[id("PvP team ID (40 hexadecimal characters).")],
    },
];

impl Param {
    fn schema(&self) -> Value {
        let mut schema = match self.kind {
            Kind::Id | Kind::Text => json!({ "type": "string" }),
            Kind::Page => json!({ "type": "integer", "minimum": 1 }),
            Kind::Include(sections) => json!({
                "type": "array",
                "items": { "type": "string", "enum": sections },
                "uniqueItems": true,
            }),
        };
        schema["description"] = self.description.into();
        schema
    }
}

impl Tool {
    fn describe(&self) -> Value {
        let properties: serde_json::Map<_, _> = self
            .params
            .iter()
            .map(|p| (p.name.to_owned(), p.schema()))
            .collect();
        let required: Vec<_> = self
            .params
            .iter()
            .filter(|p| p.required)
            .map(|p| p.name)
            .collect();
        json!({
            "name": self.name,
            "description": self.description,
            "inputSchema": {
                "type": "object",
                "properties": properties,
                "required": required,
                "additionalProperties": false,
            },
            "annotations": {
                "title": self.name,
                "readOnlyHint": true,
                "idempotentHint": true,
                "openWorldHint": true,
            },
        })
    }

    /// Turns tool arguments into a request URI, or says what is wrong with them.
    fn uri(&self, args: &Value) -> Result<String, String> {
        let args = args.as_object().ok_or("arguments must be an object")?;
        if let Some(unknown) = args
            .keys()
            .find(|k| !self.params.iter().any(|p| p.name == k.as_str()))
        {
            return Err(format!("unknown argument `{unknown}`"));
        }

        let mut path = self.path.to_owned();
        let mut query = form_urlencoded::Serializer::new(String::new());
        for p in self.params {
            let Some(value) = args.get(p.name).filter(|v| !v.is_null()) else {
                if p.required {
                    return Err(format!("missing required argument `{}`", p.name));
                }
                continue;
            };
            let value = p.encode(value)?;
            if p.in_path {
                path = path.replace(&format!("{{{}}}", p.name), &value);
            } else {
                query.append_pair(p.name, &value);
            }
        }
        let query = query.finish();
        Ok(if query.is_empty() {
            path
        } else {
            format!("{path}?{query}")
        })
    }
}

impl Param {
    fn encode(&self, value: &Value) -> Result<String, String> {
        let bad = |want: &str| format!("`{}` must be {want}", self.name);
        match (self.kind, value) {
            // IDs are alphanumeric; refusing everything else keeps path
            // segments (`/`, `?`, `..`) out of the URI.
            (Kind::Id, Value::String(s)) if s.bytes().all(|b| b.is_ascii_alphanumeric()) => {
                Ok(s.clone())
            }
            (Kind::Id | Kind::Page, Value::Number(n)) if n.is_u64() => Ok(n.to_string()),
            (Kind::Id, _) => Err(bad("an alphanumeric ID")),
            (Kind::Text, Value::String(s)) => Ok(s.clone()),
            (Kind::Text, _) => Err(bad("a string")),
            (Kind::Page, _) => Err(bad("a positive integer")),
            (Kind::Include(allowed), Value::Array(items)) => {
                let mut names = Vec::with_capacity(items.len());
                for item in items {
                    match item.as_str() {
                        Some(s) if allowed.contains(&s) => names.push(s),
                        _ => return Err(bad(&format!("a list of: {}", allowed.join(", ")))),
                    }
                }
                Ok(names.join(","))
            }
            (Kind::Include(allowed), _) => Err(bad(&format!("a list of: {}", allowed.join(", ")))),
        }
    }
}

#[derive(Deserialize)]
struct RpcRequest {
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

fn result(id: &Value, result: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: &Value, code: i32, message: impl Into<String>) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message.into() } })
}

fn tool_result(text: &str, is_error: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

/// Runs one tool through the application router.
async fn call_tool(api: AxumRouter, params: &Value) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or("missing tool name")?;
    let tool = TOOLS
        .iter()
        .find(|t| t.name == name)
        .ok_or_else(|| format!("unknown tool `{name}`"))?;

    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let uri = match tool.uri(&args) {
        Ok(uri) => uri,
        Err(message) => return Ok(tool_result(&message, true)),
    };

    let request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header(header::ACCEPT, "application/json")
        .body(Body::empty())
        .map_err(|e| e.to_string())?;
    let Ok(response) = api.oneshot(request).await;
    let status = response.status();
    let body = to_bytes(response.into_body(), MAX_RESULT_BYTES)
        .await
        .map_err(|e| format!("could not read response: {e}"))?;
    let body = String::from_utf8_lossy(&body).into_owned();

    Ok(if status.is_success() {
        tool_result(&body, false)
    } else {
        tool_result(&format!("HTTP {}: {body}", status.as_u16()), true)
    })
}

/// Handles one JSON-RPC message; `None` for notifications (no reply).
async fn dispatch(api: AxumRouter, req: RpcRequest) -> Option<Value> {
    let id = req.id?;
    Some(match req.method.as_str() {
        "initialize" => {
            let version = req
                .params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .filter(|v| SUPPORTED_PROTOCOLS.contains(v))
                .unwrap_or(LATEST_PROTOCOL);
            result(
                &id,
                &json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": {
                        "name": env!("CARGO_PKG_NAME"),
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                    "instructions": "Read-only access to the FFXIV Lodestone (characters, free \
                        companies, linkshells, PvP teams). Search first to find an ID, then use \
                        the matching get_* tool. IDs are strings.",
                }),
            )
        }
        "ping" => result(&id, &json!({})),
        "tools/list" => result(
            &id,
            &json!({ "tools": TOOLS.iter().map(Tool::describe).collect::<Vec<_>>() }),
        ),
        "tools/call" => match call_tool(api, &req.params).await {
            Ok(value) => result(&id, &value),
            Err(message) => error(&id, -32602, message),
        },
        other => error(&id, -32601, format!("method not found: {other}")),
    })
}

async fn handle(State(api): State<AxumRouter>, body: axum::body::Bytes) -> Response {
    let req: RpcRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            return Json(error(&Value::Null, -32700, format!("parse error: {e}"))).into_response()
        }
    };
    // Notifications are acknowledged with no body.
    dispatch(api, req).await.map_or_else(
        || StatusCode::ACCEPTED.into_response(),
        |reply| Json(reply).into_response(),
    )
}

/// Mounts `POST /mcp`. Call after every other route is registered: the tools
/// run against a snapshot of `router`.
pub fn mount(router: AxumRouter) -> AxumRouter {
    let mcp = AxumRouter::new()
        .route("/mcp", post(handle))
        .with_state(router.clone());
    router.merge(mcp)
}
