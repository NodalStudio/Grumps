// crates/worker/src/routes/stripe_webhook.rs
use crate::db;
use worker::*;

/// POST /webhook/stripe — handle Stripe events
pub async fn handle_stripe_webhook(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let body = req.text().await?;

    // Parse the event (simplified — in production, verify Stripe signature)
    let event: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| Error::RustError(format!("Invalid JSON: {}", e)))?;

    let event_type = event
        .pointer("/type")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    console_log!("Stripe event: {}", event_type);

    let index_db = db::get_index_db(&ctx.env)?;

    match event_type {
        "checkout.session.completed" => {
            // Customer subscribed — upgrade plan
            let slug = event
                .pointer("/data/object/metadata/workspace_slug")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let plan = event
                .pointer("/data/object/metadata/plan")
                .and_then(|v| v.as_str())
                .unwrap_or("pro");
            let _stripe_customer_id = event
                .pointer("/data/object/customer")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if !slug.is_empty() {
                index_db
                    .prepare("UPDATE workspaces_meta SET plan = ?1 WHERE slug = ?2")
                    .bind(&[plan.into(), slug.into()])?
                    .run()
                    .await?;
                console_log!("Upgraded workspace {} to plan {}", slug, plan);
            }
        }
        "customer.subscription.deleted" | "customer.subscription.updated" => {
            // Subscription cancelled or changed
            let status = event
                .pointer("/data/object/status")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let slug = event
                .pointer("/data/object/metadata/workspace_slug")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if status == "canceled" || status == "unpaid" {
                if !slug.is_empty() {
                    index_db
                        .prepare("UPDATE workspaces_meta SET plan = 'free' WHERE slug = ?1")
                        .bind(&[slug.into()])?
                        .run()
                        .await?;
                    console_log!("Downgraded workspace {} to free", slug);
                }
            }
        }
        _ => {
            console_log!("Unhandled Stripe event: {}", event_type);
        }
    }

    Response::ok("ok")
}
