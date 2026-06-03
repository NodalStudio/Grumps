//! Demo mode. When the SPA is loaded under `/demo/...` or with `?demo=1`,
//! every API call is short-circuited to deterministic seed data and the
//! auth gate is bypassed. This lets the landing page embed the *real*
//! SPA as its demo iframe — a single source of truth instead of a
//! hand-maintained HTML mockup.

use crate::api::{
    ActivityItem, CalendarItem, EventItem, MemberItem, MemoryItem, NoteItem, ScheduledActionItem,
    StatusCounts, TodoItem, WorkspaceInfo, WorkspaceSettings,
};

/// `true` when the current page is running in demo mode. Checks for
/// `/demo` anywhere in the pathname (so it works whether the SPA is
/// mounted at `/demo/` on a custom domain or at `/Grumps/demo/` on GH
/// Pages) and for `?demo=1` in the query.
pub fn is_demo() -> bool {
    let Some(win) = web_sys::window() else {
        return false;
    };
    let loc = win.location();
    if let Ok(pathname) = loc.pathname() {
        if pathname.contains("/demo") {
            return true;
        }
    }
    if let Ok(search) = loc.search() {
        if search.contains("demo=1") || search.contains("demo=true") {
            return true;
        }
    }
    false
}

/// Returns the URL prefix to prepend to all SPA routes when running in
/// demo mode — derived from the actual `window.location.pathname`. For
/// example, on GH Pages serving `/Grumps/demo/...` this returns
/// `"/Grumps/demo"`; on a custom domain serving `/demo/...` it returns
/// `"/demo"`. Empty string when not in demo mode.
pub fn router_base() -> String {
    if !is_demo() {
        return String::new();
    }
    let Some(win) = web_sys::window() else {
        return String::new();
    };
    let pathname = win.location().pathname().unwrap_or_default();
    // Find `/demo` and keep everything up to and including it.
    if let Some(idx) = pathname.find("/demo") {
        return pathname[..idx + "/demo".len()].to_string();
    }
    "/demo".to_string()
}

pub const DEMO_SLUG: &str = "roommates";
pub const DEMO_MEMBER_ID: &str = "seed-member-alice";
pub const DEMO_TOKEN: &str = "demo";

pub fn workspaces() -> Vec<WorkspaceInfo> {
    vec![
        WorkspaceInfo {
            slug: DEMO_SLUG.into(),
            name: Some("seed.workspace.name".into()),
            role: "admin".into(),
        },
        WorkspaceInfo {
            slug: "trip".into(),
            name: Some("seed.workspace2.name".into()),
            role: "member".into(),
        },
    ]
}

pub fn status_counts() -> StatusCounts {
    StatusCounts {
        open_todos: 12,
        done_this_week: 8,
        notes: 7,
        files: 9,
    }
}

pub fn members() -> Vec<MemberItem> {
    vec![
        member(DEMO_MEMBER_ID, "+33600000001", "Alice Martin", "admin"),
        member("seed-member-bob", "+33600000002", "Bob", "member"),
        member("seed-member-marie", "+33600000003", "Marie", "member"),
        member("seed-member-tom", "+33600000004", "Tom", "member"),
    ]
}
fn member(id: &str, phone: &str, name: &str, role: &str) -> MemberItem {
    MemberItem {
        id: id.into(),
        platform_user_id: phone.into(),
        display_name: Some(name.into()),
        role: role.into(),
    }
}

pub fn todos() -> Vec<TodoItem> {
    vec![
        todo(23, "seed.todo.buy_toilet_paper", 1, "Bob", "open"),
        todo(24, "seed.todo.pay_electricity", 2, "Alice", "open"),
        todo(25, "seed.todo.call_plumber", 2, "Alice", "in_progress"),
        todo(26, "seed.todo.gift_for_eve", 1, "Alice", "open"),
        todo(27, "seed.todo.clean_kitchen", 0, "Marie", "open"),
        todo(28, "seed.todo.water_plants", 1, "Marie", "open"),
        todo(29, "seed.todo.landlord_email", 2, "Bob", "open"),
        todo(30, "seed.todo.groceries", 1, "Tom", "open"),
    ]
}
fn todo(seq: i64, key: &str, priority: i32, assignee: &str, status: &str) -> TodoItem {
    TodoItem {
        id: format!("seed-todo-{}", seq),
        seq_num: seq,
        title: key.into(),
        status: status.into(),
        assigned_name: Some(assignee.into()),
        priority,
        tags: String::new(),
    }
}

pub fn notes() -> Vec<NoteItem> {
    vec![
        note(
            "seed-note-1",
            "seed.note.wifi.title",
            "seed.note.wifi.content",
            1,
        ),
        note(
            "seed-note-2",
            "seed.note.house_rules.title",
            "seed.note.house_rules.content",
            1,
        ),
        note(
            "seed-note-3",
            "seed.note.trash.title",
            "seed.note.trash.content",
            0,
        ),
        note(
            "seed-note-4",
            "seed.note.landlord.title",
            "seed.note.landlord.content",
            0,
        ),
        note(
            "seed-note-5",
            "seed.note.emergency.title",
            "seed.note.emergency.content",
            0,
        ),
    ]
}
fn note(id: &str, title: &str, content: &str, pinned: i32) -> NoteItem {
    NoteItem {
        id: id.into(),
        title: Some(title.into()),
        content: Some(content.into()),
        pinned: Some(pinned),
        source: "chat".into(),
        created_by: Some(DEMO_MEMBER_ID.into()),
        created_at: "2026-03-15T10:00:00Z".into(),
        updated_at: Some("2026-04-15T10:00:00Z".into()),
    }
}

pub fn memories() -> Vec<MemoryItem> {
    vec![
        memory(
            142,
            "preference",
            "seed.memory.bob_reminder.value",
            Some("Bob"),
            true,
        ),
        memory(
            138,
            "fact",
            "seed.memory.alice_peanuts.value",
            Some("Alice"),
            true,
        ),
        memory(
            131,
            "decision",
            "seed.memory.lisbon_trip.value",
            None,
            false,
        ),
        memory(125, "fact", "seed.memory.trash_tuesday.value", None, false),
        memory(
            119,
            "preference",
            "seed.memory.split_evenly.value",
            None,
            true,
        ),
        memory(
            114,
            "decision",
            "seed.memory.utilities_groceries.value",
            None,
            false,
        ),
    ]
}
fn memory(id: i64, kind: &str, value_key: &str, related: Option<&str>, pinned: bool) -> MemoryItem {
    MemoryItem {
        id: format!("seed-mem-{id}"),
        key: None,
        value: value_key.into(),
        kind: kind.into(),
        related_member: related.map(|s| s.into()),
        source: "chat_auto".into(),
        confidence: 0.92,
        pinned,
        expires_at: None,
        created_by: Some(DEMO_MEMBER_ID.into()),
        created_at: "2026-03-15T10:00:00Z".into(),
        updated_at: "2026-04-15T10:00:00Z".into(),
    }
}

pub fn events() -> Vec<EventItem> {
    vec![
        event(
            "seed-evt-1",
            "seed.event.gas_inspection.title",
            "2026-04-21T14:00:00Z",
            Some("seed.event.gas_inspection.location"),
            "#C0392B",
        ),
        event(
            "seed-evt-2",
            "seed.event.dinner_neighbours.title",
            "2026-04-26T19:00:00Z",
            None,
            "#C0392B",
        ),
        event(
            "seed-evt-3",
            "seed.event.plumber_follow_up.title",
            "2026-04-16T10:00:00Z",
            None,
            "#1B6B5A",
        ),
    ]
}
fn event(id: &str, title: &str, start: &str, location: Option<&str>, color: &str) -> EventItem {
    EventItem {
        id: id.into(),
        title: title.into(),
        description: None,
        starts_at: start.into(),
        ends_at: None,
        all_day: false,
        location: location.map(|s| s.into()),
        recurrence: None,
        attendees: vec![],
        color: color.into(),
        created_at: "2026-04-01T00:00:00Z".into(),
        updated_at: "2026-04-01T00:00:00Z".into(),
    }
}

pub fn scheduled_actions() -> Vec<ScheduledActionItem> {
    vec![
        sched(
            "seed-sch-1",
            "reminder",
            "seed.sch.trash.title",
            Some("FREQ=WEEKLY;BYDAY=TU"),
            "2026-04-21T09:00:00Z",
            12,
        ),
        sched(
            "seed-sch-2",
            "follow_up",
            "seed.sch.landlord.title",
            None,
            "2026-04-24T18:00:00Z",
            0,
        ),
        sched(
            "seed-sch-3",
            "reminder",
            "seed.sch.gas_remind.title",
            None,
            "2026-04-19T18:00:00Z",
            0,
        ),
        sched(
            "seed-sch-4",
            "reminder",
            "seed.sch.rent.title",
            Some("FREQ=MONTHLY;BYMONTHDAY=1"),
            "2026-05-01T09:00:00Z",
            3,
        ),
        sched(
            "seed-sch-5",
            "agent_task",
            "seed.sch.plants.title",
            Some("FREQ=WEEKLY;BYDAY=WE"),
            "2026-04-22T09:00:00Z",
            4,
        ),
    ]
}
fn sched(
    id: &str,
    action_type: &str,
    title: &str,
    rrule: Option<&str>,
    trigger: &str,
    fires: i64,
) -> ScheduledActionItem {
    ScheduledActionItem {
        id: id.into(),
        action_type: action_type.into(),
        title: title.into(),
        trigger_at: trigger.into(),
        recurrence: rrule.map(|s| s.into()),
        status: "active".into(),
        fire_count: fires,
        created_at: "2026-04-01T00:00:00Z".into(),
    }
}

pub fn calendar_items() -> Vec<CalendarItem> {
    vec![
        cal(
            "seed-cal-1",
            "event",
            "seed.event.gas_inspection.title",
            "2026-04-21T14:00:00Z",
            false,
            Some("seed.event.gas_inspection.location"),
            "#C0392B",
            None,
            None,
        ),
        cal(
            "seed-cal-2",
            "todo",
            "seed.todo.pay_electricity",
            "2026-04-24T00:00:00Z",
            true,
            None,
            "#1B6B5A",
            Some("seed-member-alice"),
            None,
        ),
        cal(
            "seed-cal-3",
            "scheduled",
            "seed.sch.trash.title",
            "2026-04-21T09:00:00Z",
            false,
            None,
            "#D4940A",
            Some("seed-member-bob"),
            Some("FREQ=WEEKLY;BYDAY=TU"),
        ),
        cal(
            "seed-cal-4",
            "event",
            "seed.event.dinner_neighbours.title",
            "2026-04-26T19:00:00Z",
            false,
            None,
            "#C0392B",
            None,
            None,
        ),
        cal(
            "seed-cal-5",
            "todo",
            "seed.todo.groceries",
            "2026-04-21T00:00:00Z",
            true,
            None,
            "#1B6B5A",
            Some("seed-member-tom"),
            None,
        ),
    ]
}
fn cal(
    id: &str,
    source: &str,
    title: &str,
    starts: &str,
    all_day: bool,
    location: Option<&str>,
    color: &str,
    member: Option<&str>,
    recurrence: Option<&str>,
) -> CalendarItem {
    CalendarItem {
        id: id.into(),
        source: source.into(),
        title: title.into(),
        starts_at: starts.into(),
        ends_at: None,
        all_day,
        location: location.map(|s| s.into()),
        color: color.into(),
        member_id: member.map(|s| s.into()),
        recurrence: recurrence.map(|s| s.into()),
        editable: true,
        url: String::new(),
    }
}

pub fn activity() -> Vec<ActivityItem> {
    vec![
        act(
            "seed-act-1",
            "Bob",
            "todo.completed",
            Some("seed.todo.buy_toilet_paper"),
            "2026-04-20T10:48:00Z",
            "chat",
        ),
        act(
            "seed-act-2",
            "Alice",
            "todo.created",
            Some("seed.todo.call_plumber"),
            "2026-04-20T09:30:00Z",
            "chat",
        ),
        act(
            "seed-act-3",
            "Claire",
            "file.uploaded",
            Some("seed.file.electricity_bill"),
            "2026-04-20T07:15:00Z",
            "chat",
        ),
        act(
            "seed-act-4",
            "Alice",
            "note.edited",
            Some("seed.note.wifi.title"),
            "2026-04-19T19:00:00Z",
            "web",
        ),
        act(
            "seed-act-5",
            "Dan",
            "todo.created",
            Some("seed.todo.gift_for_eve"),
            "2026-04-19T11:00:00Z",
            "chat",
        ),
    ]
}
fn act(
    id: &str,
    actor: &str,
    action: &str,
    target_key: Option<&str>,
    at: &str,
    source: &str,
) -> ActivityItem {
    ActivityItem {
        id: id.into(),
        actor: Some(actor.into()),
        action: action.into(),
        target_type: Some("todo".into()),
        target_id: target_key.map(|s| s.into()),
        source: source.into(),
        created_at: at.into(),
    }
}

pub fn settings() -> WorkspaceSettings {
    WorkspaceSettings {
        language: Some("en".into()),
        timezone: Some("Europe/Paris".into()),
        quiet_mode: Some(false),
        auto_recap: Some(true),
        persona: Some("default".into()),
        proactive_mode: Some(false),
        auto_memory: Some(true),
        ical_token: None,
        agent_calls_used: Some(47),
        agent_calls_limit: Some(1000),
        web_search_used: Some(12),
        web_search_limit: Some(50),
        storage_used_mb: Some(24.0),
        storage_limit_mb: Some(5120.0),
    }
}

// Optimistic mutation stubs.
pub fn new_todo(title: &str, priority: i32) -> TodoItem {
    TodoItem {
        id: "seed-todo-new".into(),
        seq_num: 99,
        title: title.into(),
        status: "open".into(),
        assigned_name: None,
        priority,
        tags: String::new(),
    }
}
pub fn new_note(title: &str, content: &str) -> NoteItem {
    NoteItem {
        id: "seed-note-new".into(),
        title: Some(title.into()),
        content: Some(content.into()),
        pinned: Some(0),
        source: "web".into(),
        created_by: Some(DEMO_MEMBER_ID.into()),
        created_at: "2026-04-20T12:00:00Z".into(),
        updated_at: Some("2026-04-20T12:00:00Z".into()),
    }
}
pub fn new_memory(body: &serde_json::Value) -> MemoryItem {
    MemoryItem {
        id: "seed-mem-new".into(),
        key: body.get("key").and_then(|v| v.as_str()).map(String::from),
        value: body
            .get("value")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        kind: body
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("fact")
            .to_string(),
        related_member: body
            .get("related_member")
            .and_then(|v| v.as_str())
            .map(String::from),
        source: "web".into(),
        confidence: 1.0,
        pinned: false,
        expires_at: None,
        created_by: Some(DEMO_MEMBER_ID.into()),
        created_at: "2026-04-20T12:00:00Z".into(),
        updated_at: "2026-04-20T12:00:00Z".into(),
    }
}
pub fn new_event(body: &serde_json::Value) -> EventItem {
    EventItem {
        id: "seed-evt-new".into(),
        title: body
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        description: None,
        starts_at: body
            .get("starts_at")
            .and_then(|v| v.as_str())
            .unwrap_or("2026-04-20T12:00:00Z")
            .to_string(),
        ends_at: None,
        all_day: false,
        location: None,
        recurrence: None,
        attendees: vec![],
        color: "#C0392B".into(),
        created_at: "2026-04-20T12:00:00Z".into(),
        updated_at: "2026-04-20T12:00:00Z".into(),
    }
}
pub fn new_scheduled(body: &serde_json::Value) -> ScheduledActionItem {
    ScheduledActionItem {
        id: "seed-sch-new".into(),
        action_type: body
            .get("action_type")
            .and_then(|v| v.as_str())
            .unwrap_or("reminder")
            .to_string(),
        title: body
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        trigger_at: body
            .get("trigger_at")
            .and_then(|v| v.as_str())
            .unwrap_or("2026-04-20T12:00:00Z")
            .to_string(),
        recurrence: body
            .get("recurrence")
            .and_then(|v| v.as_str())
            .map(String::from),
        status: "active".into(),
        fire_count: 0,
        created_at: "2026-04-20T12:00:00Z".into(),
    }
}

/// Install a `postMessage` listener so the surrounding landing page
/// (which embeds the SPA in an iframe) can drive client-side navigation
/// inside the SPA without triggering a full reload — and the static
/// dev server returning 404 on the deep URL. Expected payload:
/// `{ type: "grumps:navigate", page: "calendar" | "memory" | ... }`.
pub fn install_postmessage_nav() {
    use wasm_bindgen::prelude::Closure;
    use wasm_bindgen::JsCast;

    let Some(win) = web_sys::window() else {
        return;
    };
    let cb = Closure::wrap(Box::new(move |ev: web_sys::MessageEvent| {
        let Some(win) = web_sys::window() else {
            return;
        };
        if let Ok(origin) = win.location().origin() {
            if ev.origin() != origin {
                return;
            }
        }
        let data = ev.data();
        let Some(obj) = data.dyn_ref::<js_sys::Object>() else {
            return;
        };
        let type_v =
            js_sys::Reflect::get(obj, &"type".into()).unwrap_or(wasm_bindgen::JsValue::NULL);
        if type_v.as_string().as_deref() != Some("grumps:navigate") {
            return;
        }
        let page = js_sys::Reflect::get(obj, &"page".into())
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        let base = router_base();
        let path = match page.as_str() {
            "overview" => format!("{}/w/{}", base, DEMO_SLUG),
            "todos" => format!("{}/w/{}/todos", base, DEMO_SLUG),
            "notes" => format!("{}/w/{}/notes", base, DEMO_SLUG),
            "files" => format!("{}/w/{}/files", base, DEMO_SLUG),
            "history" => format!("{}/w/{}/history", base, DEMO_SLUG),
            "calendar" => format!("{}/w/{}/calendar", base, DEMO_SLUG),
            "memory" => format!("{}/w/{}/memory", base, DEMO_SLUG),
            "scheduled" => format!("{}/w/{}/scheduled", base, DEMO_SLUG),
            "settings" => format!("{}/w/{}/settings", base, DEMO_SLUG),
            _ => return,
        };
        // Drive the router via History API + popstate (works without a
        // Leptos runtime context — router listens for popstate).
        if let Ok(history) = win.history() {
            let _ = history.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&path));
            if let Ok(event) = web_sys::Event::new("popstate") {
                let _ = win.dispatch_event(&event);
            }
        }
    }) as Box<dyn FnMut(_)>);

    let _ = win.add_event_listener_with_callback("message", cb.as_ref().unchecked_ref());
    cb.forget();
}
