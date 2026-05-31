//! Integration test : create scheduled action with trigger_at = now+90s,
//! wait, verify DO fires it (message arrives in test channel).
//! Requires the same setup as integration_memory.rs.
//! Run : cargo test --target x86_64-pc-windows-msvc --test integration_scheduled -- --ignored --nocapture

use serde_json::json;
use std::thread;
use std::time::Duration;

const BASE: &str = "http://localhost:8787";

fn jwt() -> String {
    std::env::var("GRUMPS_TEST_JWT").expect("set GRUMPS_TEST_JWT")
}
fn slug() -> String {
    std::env::var("GRUMPS_TEST_SLUG").expect("set GRUMPS_TEST_SLUG")
}

#[test]
#[ignore]
fn create_reminder_fires_via_do_alarm() {
    let client = reqwest::blocking::Client::new();
    let auth = format!("Bearer {}", jwt());

    // Trigger 90s in future — short enough for test, long enough for DO arming
    let trigger = chrono::Utc::now() + chrono::Duration::seconds(90);
    let resp = client
        .post(format!("{BASE}/api/w/{}/scheduled", slug()))
        .header("authorization", &auth)
        .json(&json!({
            "action_type": "reminder",
            "title": "Test reminder",
            "trigger_at": trigger.to_rfc3339(),
            "payload": { "text": "Test reminder fired!" }
        }))
        .send()
        .unwrap();
    assert_eq!(resp.status(), 201);
    let created: serde_json::Value = resp.json().unwrap();
    let id = created["id"].as_str().unwrap().to_string();

    println!("Action created: {id}, waiting 100s for DO alarm...");
    thread::sleep(Duration::from_secs(100));

    // Status should now be 'done'
    let resp = client
        .get(format!("{BASE}/api/w/{}/scheduled/{id}", slug()))
        .header("authorization", &auth)
        .send()
        .unwrap();
    assert_eq!(resp.status(), 200);
    let action: serde_json::Value = resp.json().unwrap();
    assert_eq!(
        action["status"], "done",
        "action did not fire (status: {:?})",
        action["status"]
    );
    assert!(action["last_fired_at"].is_string());

    // Manually verify in the test chat that "Test reminder fired!" was received.
    println!("✓ Action marked done. Verify message in test chat manually.");
}
