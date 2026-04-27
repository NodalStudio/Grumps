//! Cross-workspace observability dashboard — super admin only.
//! Route: /admin/observability

use leptos::prelude::*;
use leptos_router::components::A;
use crate::api::ApiClient;
use crate::api::{GlobalObservabilityData, GlobalWorkspaceStats, GlobalModelCostAgg, QualitySignalCount, GlobalError};
use crate::i18n::tr;

// ── helpers ───────────────────────────────────────────────────────────────────

fn fmt_usd(v: f64) -> String {
    format!("${:.4}", v)
}

fn provider_color(provider: &str) -> &'static str {
    match provider {
        "anthropic" => "var(--brick)",
        "gemini"    => "var(--teal)",
        _           => "var(--ink-40)",
    }
}

fn signal_label(s: &str) -> (&'static str, &'static str) {
    match s {
        "praise"          => ("Praise",     "var(--teal)"),
        "thanks"          => ("Thanks",     "var(--teal)"),
        "silence_request" => ("Silence",    "var(--brick)"),
        "forget_request"  => ("Forget",     "var(--ochre)"),
        "correction"      => ("Correction", "var(--brick)"),
        "confusion"       => ("Confusion",  "var(--ochre)"),
        _                 => ("Other",      "var(--ink-40)"),
    }
}

// ── Hero stat card ────────────────────────────────────────────────────────────

#[component]
fn StatCard(label: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="flex-1 min-w-[160px] border-2 border-ink p-4"
             style="background: var(--cream); box-shadow: 3px 3px 0 #1A1A1A;">
            <div class="font-display text-[2.6rem] font-extrabold leading-none">{value}</div>
            <div class="text-[11px] uppercase tracking-widest font-bold mt-1" style="color: var(--ink-40);">{label}</div>
        </div>
    }
}

// ── Section wrapper ───────────────────────────────────────────────────────────

#[component]
fn Section(title: &'static str, children: Children) -> impl IntoView {
    view! {
        <section class="border-2 border-ink p-5 mb-6" style="box-shadow: 3px 3px 0 #1A1A1A; background: var(--cream);">
            <h2 class="font-display text-xl font-extrabold uppercase tracking-tight mb-4 border-b-2 border-ink pb-2">{title}</h2>
            {children()}
        </section>
    }
}

// ── Cost by model bar ─────────────────────────────────────────────────────────

#[component]
fn ModelCostBar(rows: Vec<GlobalModelCostAgg>, total: f64) -> impl IntoView {
    if rows.is_empty() || total == 0.0 {
        return view! { <div class="text-sm italic" style="color:var(--ink-40);">{move || tr("observability.no_data")}</div> }.into_any();
    }

    let segments: Vec<_> = rows.iter().map(|r| {
        let pct = if total > 0.0 { (r.cost_usd / total * 100.0) as u32 } else { 0 };
        let color = provider_color(&r.provider);
        (pct, color, r.model.clone(), r.cost_usd, r.call_count)
    }).collect();

    view! {
        <div>
            <div class="flex h-10 border-2 border-ink overflow-hidden mb-3">
                {segments.iter().map(|(pct, color, _m, _c, _n)| {
                    let style = format!("width: {}%; background: {}; flex-shrink:0;", pct, color);
                    view! { <div style=style></div> }
                }).collect::<Vec<_>>()}
            </div>
            <table class="w-full text-sm border-collapse">
                <thead>
                    <tr class="border-b-2 border-ink">
                        <th class="text-left font-bold py-1 pr-4 text-[11px] uppercase tracking-wider">"Model"</th>
                        <th class="text-right font-bold py-1 px-2 text-[11px] uppercase tracking-wider">"Calls"</th>
                        <th class="text-right font-bold py-1 pl-2 text-[11px] uppercase tracking-wider">"Cost"</th>
                    </tr>
                </thead>
                <tbody>
                    {rows.iter().map(|r| {
                        let dot_color = provider_color(&r.provider).to_string();
                        let model = r.model.clone();
                        let calls = r.call_count;
                        let cost = fmt_usd(r.cost_usd);
                        view! {
                            <tr class="border-b" style="border-color: var(--ink-15);">
                                <td class="py-1.5 pr-4 flex items-center gap-2">
                                    <span class="inline-block w-3 h-3 border border-ink flex-shrink-0"
                                          style=move || format!("background: {};", dot_color)></span>
                                    <span class="font-mono text-xs">{model}</span>
                                </td>
                                <td class="py-1.5 px-2 text-right font-mono">{calls}</td>
                                <td class="py-1.5 pl-2 text-right font-mono font-bold">{cost}</td>
                            </tr>
                        }
                    }).collect::<Vec<_>>()}
                </tbody>
            </table>
        </div>
    }.into_any()
}

// ── Workspaces table ──────────────────────────────────────────────────────────

#[component]
fn WorkspacesTable(rows: Vec<GlobalWorkspaceStats>) -> impl IntoView {
    if rows.is_empty() {
        return view! { <div class="text-sm italic" style="color:var(--ink-40);">{move || tr("observability.no_workspaces")}</div> }.into_any();
    }
    view! {
        <div class="overflow-x-auto">
            <table class="w-full text-sm border-collapse">
                <thead>
                    <tr class="border-b-2 border-ink">
                        <th class="text-left font-bold py-1 pr-4 text-[11px] uppercase tracking-wider">"Slug"</th>
                        <th class="text-left font-bold py-1 pr-4 text-[11px] uppercase tracking-wider">"Name"</th>
                        <th class="text-left font-bold py-1 pr-4 text-[11px] uppercase tracking-wider">"Plan"</th>
                        <th class="text-right font-bold py-1 px-2 text-[11px] uppercase tracking-wider">"Calls"</th>
                        <th class="text-right font-bold py-1 px-2 text-[11px] uppercase tracking-wider">"Cost (30d)"</th>
                        <th class="text-right font-bold py-1 px-2 text-[11px] uppercase tracking-wider">"Quality"</th>
                        <th class="text-right font-bold py-1 pl-2 text-[11px] uppercase tracking-wider">"Detail"</th>
                    </tr>
                </thead>
                <tbody>
                    {rows.iter().map(|r| {
                        let slug = r.slug.clone();
                        let name = r.name.clone().unwrap_or_else(|| "—".to_string());
                        let plan = r.plan.clone();
                        let calls = r.calls;
                        let cost = fmt_usd(r.cost_usd);
                        let quality = format!("{:.0}%", r.quality_score * 100.0);
                        let detail_href = format!("/w/{}/admin/observability", r.slug);
                        view! {
                            <tr class="border-b transition-colors hover:bg-black/[0.035]"
                                style="border-color: var(--ink-15); cursor: pointer;"
                                on:click={
                                    let href = detail_href.clone();
                                    move |_| {
                                        if let Some(win) = web_sys::window() {
                                            let _ = win.location().set_href(&href);
                                        }
                                    }
                                }>
                                <td class="py-1.5 pr-4 font-mono text-xs font-bold">{slug}</td>
                                <td class="py-1.5 pr-4 text-sm">{name}</td>
                                <td class="py-1.5 pr-4">
                                    <span class="text-[10px] uppercase font-bold px-1.5 py-0.5 border border-ink"
                                          style="background: var(--cream-light);">{plan}</span>
                                </td>
                                <td class="py-1.5 px-2 text-right font-mono">{calls}</td>
                                <td class="py-1.5 px-2 text-right font-mono font-bold">{cost}</td>
                                <td class="py-1.5 px-2 text-right font-mono">{quality}</td>
                                <td class="py-1.5 pl-2 text-right">
                                    <A href=detail_href.clone()
                                       attr:class="text-xs font-bold"
                                       attr:style="color: var(--teal);">"→"</A>
                                </td>
                            </tr>
                        }
                    }).collect::<Vec<_>>()}
                </tbody>
            </table>
        </div>
    }.into_any()
}

// ── Quality signals ───────────────────────────────────────────────────────────

#[component]
fn QualitySignals(signals: Vec<QualitySignalCount>) -> impl IntoView {
    if signals.is_empty() {
        return view! { <div class="text-sm italic" style="color:var(--ink-40);">{move || tr("observability.no_quality_signals")}</div> }.into_any();
    }
    let max_count = signals.iter().map(|s| s.count).max().unwrap_or(1).max(1);

    view! {
        <div class="space-y-2">
            {signals.iter().map(|s| {
                let (label, color) = signal_label(&s.signal_type);
                let pct = (s.count * 100 / max_count).min(100) as u32;
                let count = s.count;
                view! {
                    <div class="flex items-center gap-3">
                        <div class="w-24 text-xs font-bold" style=move || format!("color:{};", color)>{label}</div>
                        <div class="flex-1 h-5 border border-ink overflow-hidden" style="background: var(--cream-light);">
                            <div style=move || format!("width:{}%; height:100%; background:{};", pct, color)></div>
                        </div>
                        <div class="w-8 text-right font-mono text-sm font-bold">{count}</div>
                    </div>
                }
            }).collect::<Vec<_>>()}
        </div>
    }.into_any()
}

// ── Recent errors ─────────────────────────────────────────────────────────────

#[component]
fn RecentErrors(errors: Vec<GlobalError>) -> impl IntoView {
    if errors.is_empty() {
        return view! { <div class="text-sm font-medium" style="color: var(--teal);">"No errors. Clean."</div> }.into_any();
    }
    view! {
        <div class="overflow-x-auto">
            <table class="w-full text-xs border-collapse">
                <thead>
                    <tr class="border-b-2 border-ink">
                        <th class="text-left font-bold py-1 pr-3 uppercase tracking-wider">"Workspace"</th>
                        <th class="text-left font-bold py-1 pr-3 uppercase tracking-wider">"Timestamp"</th>
                        <th class="text-left font-bold py-1 pr-3 uppercase tracking-wider">"Provider"</th>
                        <th class="text-left font-bold py-1 pr-3 uppercase tracking-wider">"Model"</th>
                        <th class="text-left font-bold py-1 uppercase tracking-wider">"Error"</th>
                    </tr>
                </thead>
                <tbody>
                    {errors.iter().map(|e| {
                        let ws = e.workspace_slug.clone();
                        let ts = e.created_at.clone();
                        let prov = e.provider.clone();
                        let model = e.model.clone();
                        let err = e.error.clone();
                        view! {
                            <tr class="border-b" style="border-color:var(--ink-15);">
                                <td class="py-1.5 pr-3 font-mono font-bold">{ws}</td>
                                <td class="py-1.5 pr-3 font-mono whitespace-nowrap">{ts}</td>
                                <td class="py-1.5 pr-3">{prov}</td>
                                <td class="py-1.5 pr-3 font-mono">{model}</td>
                                <td class="py-1.5 text-brick truncate max-w-[300px]">{err}</td>
                            </tr>
                        }
                    }).collect::<Vec<_>>()}
                </tbody>
            </table>
        </div>
    }.into_any()
}

// ── Page ──────────────────────────────────────────────────────────────────────

#[component]
pub fn GlobalObservabilityPage() -> impl IntoView {

    let api = ApiClient::new();
    let data = LocalResource::new(move || {
        let api = api.clone();
        async move { api.get_global_observability().await.ok() }
    });

    view! {
        <div class="flex min-h-screen" style="background: var(--cream-light);">
            // Minimal sidebar for global admin page
            <aside class="w-56 min-w-[220px] flex flex-col border-r-2 border-ink" style="background: var(--cream-light);">
                <div class="px-5 pt-6 pb-5 border-b-2 border-ink">
                    <h1 class="font-display text-xl font-extrabold uppercase tracking-tight">
                        "GRUMPS"<span class="text-brick">"."</span>
                    </h1>
                    <p class="text-[10px] uppercase tracking-wider mt-0.5 font-medium" style="color: var(--ink-40);">
                        "Super Admin"
                    </p>
                </div>
                <nav class="py-3 flex-1">
                    <div class="px-5 pt-4 pb-1.5 text-[10px] font-bold uppercase tracking-[1.5px]" style="color: var(--ink-40);">"Admin"</div>
                    <A href="/admin/observability"
                       attr:class="flex items-center gap-2.5 px-5 py-2 text-sm font-medium border-l-[3px] border-brick"
                       attr:style="color: var(--ink);">
                        <span class="w-[18px] text-center text-[15px]">{"\u{2295}"}</span>
                        "Observabilité globale"
                    </A>
                    <A href="/dashboard"
                       attr:class="flex items-center gap-2.5 px-5 py-2 text-sm font-medium border-l-[3px] border-transparent hover:bg-black/[0.04]"
                       attr:style="color: var(--ink-70);">
                        <span class="w-[18px] text-center text-[15px]">"⊞"</span>
                        "My Workspaces"
                    </A>
                </nav>
            </aside>

            // Main content
            <div class="flex-1 overflow-y-auto p-6 md:p-8">
                {move || {
                    let maybe = data.get().map(|w| (*w).clone());
                    match maybe {
                        None => view! {
                            <div class="font-display text-xl animate-pulse" style="color:var(--ink-40);">{move || tr("common.loading")}</div>
                        }.into_any(),
                        Some(None) => view! {
                            <div class="border-2 border-ink p-6" style="box-shadow:3px 3px 0 #1A1A1A; background:var(--cream);">
                                <div class="font-display text-xl font-extrabold text-brick">{move || tr("observability.access_denied")}</div>
                                <p class="text-sm mt-2" style="color:var(--ink-70);">{move || tr("observability.super_admin_only")}</p>
                            </div>
                        }.into_any(),
                        Some(Some(d)) => {
                            let d: GlobalObservabilityData = d;
                            let avg_quality = if d.by_workspace.is_empty() {
                                1.0f64
                            } else {
                                d.by_workspace.iter().map(|w| w.quality_score).sum::<f64>() / d.by_workspace.len() as f64
                            };
                            view! {
                                <div>
                                    // Header
                                    <div class="mb-8">
                                        <h1 class="font-display text-[2.8rem] font-extrabold uppercase tracking-tight leading-none">
                                            "Global observability"
                                            <span class="text-brick">"."</span>
                                        </h1>
                                        <p class="text-sm font-medium uppercase tracking-widest mt-1" style="color:var(--ink-40);">
                                            {d.generated_at.clone()}
                                        </p>
                                    </div>

                                    // Hero stats
                                    <div class="flex flex-wrap gap-4 mb-8">
                                        <StatCard label="Workspaces" value=d.workspaces_count.to_string() />
                                        <StatCard label="Total cost (30d)" value=fmt_usd(d.total_cost_usd) />
                                        <StatCard label="Total calls" value=d.total_calls.to_string() />
                                        <StatCard label="Avg quality score" value=format!("{:.0}%", avg_quality * 100.0) />
                                    </div>

                                    // Cost by model
                                    <Section title="Cost by model (30d)">
                                        <ModelCostBar rows=d.cost_by_model.clone() total=d.total_cost_usd />
                                    </Section>

                                    // Top workspaces by cost
                                    <Section title="Top workspaces by cost">
                                        <WorkspacesTable rows=d.by_workspace.clone() />
                                    </Section>

                                    // Quality signals
                                    <Section title="Quality signals (30d)">
                                        <QualitySignals signals=d.quality_signals.clone() />
                                    </Section>

                                    // Recent errors
                                    <Section title="Recent errors (all workspaces)">
                                        <RecentErrors errors=d.recent_errors.clone() />
                                    </Section>
                                </div>
                            }.into_any()
                        }
                    }
                }}
            </div>
        </div>
    }
}
