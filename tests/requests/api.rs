use loco_rs::testing::prelude::*;
use ruststone::app::App;
use serde_json::{json, Value};
use serial_test::serial;

use super::fixtures::Upstream;

#[tokio::test]
#[serial]
async fn character_profile_is_parsed_and_typed() {
    let upstream = Upstream::start().await;
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/api/characters/1").await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        let body: Value = res.json();
        let c = &body["character"];
        assert_eq!(c["id"], 1);
        assert_eq!(c["name"], "Test Character");
        assert_eq!(c["title"], "The Tester");
        // `&nbsp;` does not match the inner-HTML regex; the text fallback does.
        assert_eq!(
            c["world"],
            json!({ "name": "Gilgamesh", "data_center": "Aether" })
        );
        assert_eq!(c["race"], "Au Ra");
        assert_eq!(c["tribe"], "Raen");
        assert_eq!(c["gender"], "female");
        assert_eq!(c["free_company"]["id"], 9_229_142_273_877_347_144_u64);
        assert_eq!(c["free_company"]["name"], "Test & Co");
        assert_eq!(c["bio"], "Hello\nWorld");
        assert!(
            body.get("class_jobs").is_none(),
            "sections are absent unless included"
        );
    })
    .await;
    assert_eq!(upstream.count("/lodestone/character/1/"), 1);
}

#[tokio::test]
#[serial]
async fn repeated_requests_are_served_from_cache() {
    let upstream = Upstream::start().await;
    request::<App, _, _>(|request, _ctx| async move {
        for _ in 0..3 {
            let res = request.get("/api/characters/1").await;
            assert_eq!(res.status_code(), 200);
            assert_eq!(res.header("cache-control"), "public, max-age=1800");
        }
    })
    .await;
    assert_eq!(upstream.hits().len(), 1);
}

#[tokio::test]
#[serial]
async fn includes_are_fetched_and_private_sections_are_null() {
    let upstream = Upstream::start().await;
    request::<App, _, _>(|request, _ctx| async move {
        let res = request
            .get("/api/characters/1?include=class_jobs,mounts")
            .await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        let body: Value = res.json();
        let paladin = &body["class_jobs"]["jobs"][0];
        assert_eq!(paladin["job"], "paladin");
        assert_eq!(paladin["level"], 90);
        assert_eq!(paladin["exp"], json!({ "current": 1234, "max": 5000 }));
        assert_eq!(body["mounts"]["total"], 2);
        assert_eq!(body["mounts"]["items"][1]["name"], "Magitek Armor");
        // Cache-Control is the shortest TTL of what was included.
        assert_eq!(res.header("cache-control"), "public, max-age=1800");

        let res = request.get("/api/characters/3?include=class_jobs").await;
        assert_eq!(res.status_code(), 200);
        let body: Value = res.json();
        assert!(body["class_jobs"].is_null());
        assert_eq!(body["character"]["name"], "Test Character");
    })
    .await;

    let mount = upstream
        .hits()
        .into_iter()
        .find(|h| {
            h.path_and_query
                .starts_with("/lodestone/character/1/mount/")
        })
        .expect("mount page fetched");
    assert!(
        mount.user_agent.contains("iPhone"),
        "mounts need the mobile UA"
    );
}

#[tokio::test]
#[serial]
async fn upstream_errors_map_to_statuses() {
    let _upstream = Upstream::start().await;
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/api/characters/2").await;
        assert_eq!(res.status_code(), 404);
        assert_eq!(res.json::<Value>()["error"], "not_found");

        let res = request.get("/api/characters/3/class_jobs").await;
        assert_eq!(res.status_code(), 403);
        assert_eq!(res.json::<Value>()["error"], "private");

        let res = request.get("/api/characters/4").await;
        assert_eq!(res.status_code(), 503);
        assert_eq!(res.json::<Value>()["error"], "maintenance");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn invalid_input_is_rejected_before_any_upstream_call() {
    let upstream = Upstream::start().await;
    request::<App, _, _>(|request, _ctx| async move {
        for uri in [
            "/api/characters/abc",
            "/api/characters/1/achievements?page=0",
            "/api/characters/1?include=bogus",
            "/api/characters?name=",
            "/api/characters?name=x&world=Gil%26x%3D1",
            "/api/cwls/not-a-hash",
            "/api/pvp_teams/..%2F..%2Fetc",
        ] {
            let res = request.get(uri).await;
            assert_eq!(res.status_code(), 400, "{uri}: {}", res.text());
        }
    })
    .await;
    assert!(upstream.hits().is_empty());
}

#[tokio::test]
#[serial]
async fn search_encodes_query_and_paginates() {
    let upstream = Upstream::start().await;
    request::<App, _, _>(|request, _ctx| async move {
        let res = request
            .get("/api/characters?name=Y%27shtola%20Rhul&dc=Aether")
            .await;
        assert_eq!(res.status_code(), 200, "{}", res.text());
        let body: Value = res.json();
        assert_eq!(body["items"][0]["id"], 1);
        assert_eq!(body["items"][0]["world"]["name"], "Gilgamesh");
        assert_eq!(
            body["pagination"],
            json!({ "page": 1, "total_pages": 3, "next": 2, "prev": null })
        );
        assert_eq!(res.header("cache-control"), "public, max-age=300");
    })
    .await;
    assert_eq!(
        upstream.hits()[0].path_and_query,
        "/lodestone/character/?q=Y%27shtola+Rhul&worldname=_dc_Aether&page=1"
    );
}

#[tokio::test]
#[serial]
async fn mcp_lists_tools_and_calls_them_through_the_api() {
    let upstream = Upstream::start().await;
    request::<App, _, _>(|request, _ctx| async move {
        let rpc = |body: Value| {
            let request = &request;
            async move { request.post("/mcp").json(&body).await }
        };

        let init = rpc(json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18" }
        }))
        .await;
        assert_eq!(init.status_code(), 200);
        assert_eq!(
            init.json::<Value>()["result"]["protocolVersion"],
            "2025-06-18"
        );

        let note = rpc(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).await;
        assert_eq!(note.status_code(), 202);

        let list: Value = rpc(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }))
            .await
            .json();
        let tools = list["result"]["tools"].as_array().unwrap();
        assert!(tools.iter().any(|t| t["name"] == "get_character"));

        let call: Value = rpc(json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "get_character", "arguments": { "id": "1" } }
        }))
        .await
        .json();
        assert_eq!(call["result"]["isError"], false, "{call}");
        let text = call["result"]["content"][0]["text"].as_str().unwrap();
        let profile: Value = serde_json::from_str(text).unwrap();
        assert_eq!(profile["character"]["name"], "Test Character");

        // API validation surfaces as a tool error, not a protocol error.
        let bad: Value = rpc(json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": { "name": "get_character", "arguments": { "id": "../x" } }
        }))
        .await
        .json();
        assert_eq!(bad["result"]["isError"], true);

        let unknown: Value = rpc(json!({
            "jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": { "name": "nope" }
        }))
        .await
        .json();
        assert_eq!(unknown["error"]["code"], -32602);
    })
    .await;
    assert_eq!(upstream.count("/lodestone/character/1/"), 1);
}
