//! Integration test for memory CRUD via REST.
//! Requires : `wrangler dev --local` running on http://localhost:8787
//!           a test workspace exists in the local Index DB
//!           a valid JWT for that workspace in env var GRUMPS_TEST_JWT
//!
//! Run with : cargo test --target x86_64-pc-windows-msvc --test integration_memory -- --ignored --nocapture

use serde_json::json;

const BASE: &str = "http://localhost:8787";

fn jwt() -> String {
    std::env::var("GRUMPS_TEST_JWT").expect("set GRUMPS_TEST_JWT")
}

fn slug() -> String {
    std::env::var("GRUMPS_TEST_SLUG").expect("set GRUMPS_TEST_SLUG")
}

#[test]
#[ignore]
fn create_get_update_delete_memory() {
    let client = reqwest::blocking::Client::new();
    let auth = format!("Bearer {}", jwt());

    // 1. Create
    let resp = client.post(format!("{BASE}/api/w/{}/memory", slug()))
        .header("authorization", &auth)
        .json(&json!({
            "value": "wifi du bureau = XYZ123",
            "kind": "fact",
            "pinned": true
        }))
        .send().unwrap();
    assert_eq!(resp.status(), 201);
    let created: serde_json::Value = resp.json().unwrap();
    let id = created["id"].as_str().unwrap().to_string();

    // 2. Get
    let resp = client.get(format!("{BASE}/api/w/{}/memory/{id}", slug()))
        .header("authorization", &auth)
        .send().unwrap();
    assert_eq!(resp.status(), 200);
    let fetched: serde_json::Value = resp.json().unwrap();
    assert_eq!(fetched["value"], "wifi du bureau = XYZ123");
    assert_eq!(fetched["pinned"], true);

    // 3. Update
    let resp = client.put(format!("{BASE}/api/w/{}/memory/{id}", slug()))
        .header("authorization", &auth)
        .json(&json!({ "value": "wifi du bureau = ABC456" }))
        .send().unwrap();
    assert_eq!(resp.status(), 200);
    let updated: serde_json::Value = resp.json().unwrap();
    assert_eq!(updated["value"], "wifi du bureau = ABC456");

    // 4. List should include it
    let resp = client.get(format!("{BASE}/api/w/{}/memory", slug()))
        .header("authorization", &auth)
        .send().unwrap();
    assert_eq!(resp.status(), 200);
    let list: Vec<serde_json::Value> = resp.json().unwrap();
    assert!(list.iter().any(|e| e["id"] == id));

    // 5. Delete
    let resp = client.delete(format!("{BASE}/api/w/{}/memory/{id}", slug()))
        .header("authorization", &auth)
        .send().unwrap();
    assert_eq!(resp.status(), 204);

    // 6. Get should be 404
    let resp = client.get(format!("{BASE}/api/w/{}/memory/{id}", slug()))
        .header("authorization", &auth)
        .send().unwrap();
    assert_eq!(resp.status(), 404);
}
