//! Observability admin dashboard — LLM cost/latency/quality charts.
//! Route: /w/:slug/admin/observability

use crate::api::{
    use_api, LlmCostByModel, LlmLatencyByModel, ObservabilityData, QualitySignalCount,
};
use crate::i18n::{tr, tr_p};
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;

// ── helpers ───────────────────────────────────────────────────────────────────

fn fmt_usd(v: f64) -> String {
    format!("${:.4}", v)
}

fn fmt_ms(ms: i64) -> String {
    if ms >= 1000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format!("{}ms", ms)
    }
}

fn provider_color(provider: &str) -> &'static str {
    match provider {
        "anthropic" => "var(--brick)",
        "gemini" => "var(--teal)",
        _ => "var(--ink-40)",
    }
}

fn signal_label(s: &str) -> (String, &'static str) {
    // (localized label, palette token)
    let key = match s {
        "praise" => "observability.signal.praise",
        "thanks" => "observability.signal.thanks",
        "silence_request" => "observability.signal.silence",
        "forget_request" => "observability.signal.forget",
        "correction" => "observability.signal.correction",
        "confusion" => "observability.signal.confusion",
        _ => "observability.signal.other",
    };
    let color = match s {
        "praise" | "thanks" => "var(--teal)",
        "silence_request" | "correction" => "var(--brick)",
        "forget_request" | "confusion" => "var(--ochre)",
        _ => "var(--ink-40)",
    };
    (tr(key), color)
}

// ── Hero stat card ────────────────────────────────────────────────────────────

#[component]
fn StatCard(label: String, value: String) -> impl IntoView {
    view! {
        <div class="flex-1 min-w-[160px] border-2 border-ink p-4"
             style="background: var(--cream); box-shadow: 3px 3px 0 #1A1A1A;">
            <div class="font-display text-[2.6rem] font-extrabold leading-none">{value}</div>
            <div class="text-[11px] uppercase tracking-widest font-bold mt-1" style="color: var(--ink-40);">{label}</div>
        </div>
    }
}

// ── Stacked bar for cost by model ─────────────────────────────────────────────

#[component]
fn CostBar(rows: Vec<LlmCostByModel>, total: f64) -> impl IntoView {
    if rows.is_empty() || total == 0.0 {
        return view! { <div class="text-sm italic" style="color:var(--ink-40);">{move || tr("observability.no_data")}</div> }.into_any();
    }

    let segments: Vec<_> = rows
        .iter()
        .map(|r| {
            let pct = if total > 0.0 {
                (r.cost_usd / total * 100.0) as u32
            } else {
                0
            };
            let color = provider_color(&r.provider);
            (pct, color, r.model.clone(), r.cost_usd)
        })
        .collect();

    view! {
        <div>
            // Stacked bar
            <div class="flex h-10 border-2 border-ink overflow-hidden mb-3">
                {segments.iter().map(|(pct, color, _model, _cost)| {
                    let style = format!("width: {}%; background: {}; flex-shrink:0;", pct, color);
                    view! { <div style=style></div> }
                }).collect::<Vec<_>>()}
            </div>
            // Legend table
            <table class="w-full text-sm border-collapse">
                <thead>
                    <tr class="border-b-2 border-ink">
                        <th class="text-start font-bold py-1 pe-4 text-[11px] uppercase tracking-wider">{move || tr("observability.col.model")}</th>
                        <th class="text-end font-bold py-1 px-2 text-[11px] uppercase tracking-wider">{move || tr("observability.col.calls")}</th>
                        <th class="text-end font-bold py-1 ps-2 text-[11px] uppercase tracking-wider">{move || tr("observability.col.cost")}</th>
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
                                <td class="py-1.5 pe-4 flex items-center gap-2">
                                    <span class="inline-block w-3 h-3 border border-ink shrink-0"
                                          style=move || format!("background: {};", dot_color)></span>
                                    <span class="font-mono text-xs">{model}</span>
                                </td>
                                <td class="py-1.5 px-2 text-end font-mono">{calls}</td>
                                <td class="py-1.5 ps-2 text-end font-mono font-bold">{cost}</td>
                            </tr>
                        }
                    }).collect::<Vec<_>>()}
                </tbody>
            </table>
        </div>
    }.into_any()
}

// ── Latency table ─────────────────────────────────────────────────────────────

#[component]
fn LatencyTable(rows: Vec<LlmLatencyByModel>) -> impl IntoView {
    if rows.is_empty() {
        return view! { <div class="text-sm italic" style="color:var(--ink-40);">{move || tr("observability.no_data")}</div> }.into_any();
    }
    let max_p99 = rows.iter().map(|r| r.p99_ms).max().unwrap_or(1).max(1);

    view! {
        <table class="w-full text-sm border-collapse">
            <thead>
                <tr class="border-b-2 border-ink">
                    <th class="text-start font-bold py-1 pe-4 text-[11px] uppercase tracking-wider">{move || tr("observability.col.model")}</th>
                    <th class="text-end font-bold py-1 px-2 text-[11px] uppercase tracking-wider">{move || tr("observability.col.p50")}</th>
                    <th class="text-end font-bold py-1 px-2 text-[11px] uppercase tracking-wider">{move || tr("observability.col.p95")}</th>
                    <th class="text-end font-bold py-1 ps-2 text-[11px] uppercase tracking-wider">{move || tr("observability.col.p99")}</th>
                    <th class="text-end font-bold py-1 ps-2 text-[11px] uppercase tracking-wider">{move || tr("observability.col.count")}</th>
                </tr>
            </thead>
            <tbody>
                {rows.iter().map(|r| {
                    let model = r.model.clone();
                    let provider = r.provider.clone();
                    let p50 = fmt_ms(r.p50_ms);
                    let p95 = fmt_ms(r.p95_ms);
                    let p99 = fmt_ms(r.p99_ms);
                    let cnt = r.count;
                    let bar_pct = (r.p95_ms * 100 / max_p99).min(100) as u32;
                    let bar_color = provider_color(&provider).to_string();
                    view! {
                        <tr class="border-b" style="border-color: var(--ink-15);">
                            <td class="py-1.5 pe-4">
                                <span class="font-mono text-xs block">{model}</span>
                                <div class="mt-1 h-1.5 w-full border border-ink overflow-hidden">
                                    <div style=move || format!("width:{}%; height:100%; background:{};", bar_pct, bar_color)></div>
                                </div>
                            </td>
                            <td class="py-1.5 px-2 text-end font-mono">{p50}</td>
                            <td class="py-1.5 px-2 text-end font-mono">{p95}</td>
                            <td class="py-1.5 ps-2 text-end font-mono font-bold">{p99}</td>
                            <td class="py-1.5 ps-2 text-end font-mono">{cnt}</td>
                        </tr>
                    }
                }).collect::<Vec<_>>()}
            </tbody>
        </table>
    }.into_any()
}

// ── Donut chart (SVG) ─────────────────────────────────────────────────────────

fn donut_path(start_deg: f64, end_deg: f64, cx: f64, cy: f64, r: f64) -> String {
    let to_rad = |d: f64| d * std::f64::consts::PI / 180.0;
    let x1 = cx + r * to_rad(start_deg).cos();
    let y1 = cy + r * to_rad(start_deg).sin();
    let x2 = cx + r * to_rad(end_deg).cos();
    let y2 = cy + r * to_rad(end_deg).sin();
    let large = if end_deg - start_deg > 180.0 { 1 } else { 0 };
    format!("M {cx} {cy} L {x1:.2} {y1:.2} A {r} {r} 0 {large} 1 {x2:.2} {y2:.2} Z")
}

#[component]
fn CascadeDonut(classifier: i64, sonnet: i64, saved_usd: f64) -> impl IntoView {
    let total = (classifier + sonnet).max(1) as f64;
    let gemini_deg = classifier as f64 / total * 360.0;
    let sonnet_deg = 360.0 - gemini_deg;

    let p1 = donut_path(-90.0, -90.0 + gemini_deg, 80.0, 80.0, 70.0);
    let p2 = donut_path(-90.0 + gemini_deg, 270.0, 80.0, 80.0, 70.0);

    let gemini_pct = (classifier as f64 / total * 100.0) as u32;
    let sonnet_pct = 100 - gemini_pct;

    view! {
        <div class="flex flex-col md:flex-row items-start gap-6">
            // Donut SVG
            <svg width="160" height="160" viewBox="0 0 160 160" class="shrink-0">
                <circle cx="80" cy="80" r="70" fill="var(--cream-light)" stroke="#1A1A1A" stroke-width="2"/>
                // Gemini slice
                {if classifier > 0 {
                    view! {
                        <path d=p1 fill="#1A6B5E"/>
                    }.into_any()
                } else {
                    view! { <span></span> }.into_any()
                }}
                // Sonnet slice
                {if sonnet > 0 {
                    view! {
                        <path d=p2 fill="#C0392B"/>
                    }.into_any()
                } else {
                    view! { <span></span> }.into_any()
                }}
                // Hole
                <circle cx="80" cy="80" r="40" fill="var(--cream)" stroke="#1A1A1A" stroke-width="2"/>
                // Center label
                <text x="80" y="76" text-anchor="middle" font-size="12" font-weight="bold" fill="#1A1A1A">"GEMINI"</text>
                <text x="80" y="91" text-anchor="middle" font-size="11" fill="#1A1A1A">{gemini_pct}"%"</text>
            </svg>
            // Legend + stats
            <div class="flex-1">
                <div class="flex items-center gap-2 mb-2">
                    <span class="inline-block w-4 h-4 border border-ink" style="background: #1A6B5E;"></span>
                    <span class="text-sm font-medium">{move || tr_p("observability.cascade.resolved", &[("n", &classifier.to_string()), ("pct", &gemini_pct.to_string())])}</span>
                </div>
                <div class="flex items-center gap-2 mb-4">
                    <span class="inline-block w-4 h-4 border border-ink" style="background: #C0392B;"></span>
                    <span class="text-sm font-medium">{move || tr_p("observability.cascade.escalated", &[("n", &sonnet.to_string()), ("pct", &sonnet_pct.to_string())])}</span>
                </div>
                <div class="border-t-2 border-ink pt-3">
                    <div class="text-[11px] uppercase tracking-widest font-bold mb-1" style="color:var(--ink-40);">{move || tr("observability.cascade.estimated_savings")}</div>
                    <div class="font-display text-2xl font-extrabold">{fmt_usd(saved_usd)}</div>
                    <div class="text-xs mt-0.5" style="color:var(--ink-40);">{move || tr("observability.cascade.vs_sonnet")}</div>
                </div>
            </div>
        </div>
    }
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
                        <div class="w-8 text-end font-mono text-sm font-bold">{count}</div>
                    </div>
                }
            }).collect::<Vec<_>>()}
        </div>
    }.into_any()
}

// ── Section wrapper ───────────────────────────────────────────────────────────

#[component]
fn Section(title: String, children: Children) -> impl IntoView {
    view! {
        <section class="border-2 border-ink p-5 mb-6" style="box-shadow: 3px 3px 0 #1A1A1A; background: var(--cream);">
            <h2 class="font-display text-xl font-extrabold uppercase tracking-tight mb-4 border-b-2 border-ink pb-2">{title}</h2>
            {children()}
        </section>
    }
}

// ── Page ──────────────────────────────────────────────────────────────────────

#[component]
pub fn ObservabilityPage() -> impl IntoView {
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();

    // Super admin gate: fetch /api/admin/me and redirect if not super admin
    let api_gate = use_api();
    let gate = LocalResource::new(move || {
        let api = api_gate.clone();
        async move { api.get_admin_me().await.ok() }
    });

    let api = use_api();
    let data = LocalResource::new(move || {
        let api = api.clone();
        let s = slug();
        async move { api.get_observability(&s).await.ok() }
    });

    view! {
        <div class="flex-1 overflow-y-auto p-6 md:p-8" style="background: var(--cream-light);">
            {move || {
                // Check super admin gate first (once loaded)
                if let Some(me) = gate.get().map(|m| m.clone()) {
                    let is_super = me.map(|m| m.is_super_admin).unwrap_or(false);
                    if !is_super {
                        if let Some(win) = web_sys::window() {
                            let _ = win.location().set_href("/dashboard");
                        }
                        return view! {
                            <div class="font-display text-xl animate-pulse" style="color:var(--ink-40);">{move || tr("common.redirecting")}</div>
                        }.into_any();
                    }
                }

                let maybe = data.get().map(|w| w.clone());
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
                        let d: ObservabilityData = d;
                        let slug_label = slug();
                        view! {
                            <div>
                                // Back link
                                <div class="mb-4">
                                    <A href="/admin/observability"
                                       attr:class="text-sm font-medium"
                                       attr:style="color: var(--teal);">{move || tr("observability.back_to_global")}</A>
                                </div>
                                // Header
                                <div class="mb-8">
                                    <h1 class="font-display text-[2.8rem] font-extrabold uppercase tracking-tight leading-none">
                                        {move || tr_p("observability.title_workspace", &[("slug", &slug_label)])}
                                        <span class="text-brick">"."</span>
                                    </h1>
                                    <p class="text-[11px] uppercase tracking-[2px] font-bold mt-1" style="color: var(--brick);">
                                        {move || tr("observability.badge.super_admin_view")}
                                    </p>
                                    <p class="text-sm font-medium uppercase tracking-widest mt-1" style="color:var(--ink-40);">
                                        {d.month.clone()}
                                    </p>
                                </div>

                                // Hero stats
                                <div class="flex flex-wrap gap-4 mb-8">
                                    <StatCard label=tr("observability.stat.total_cost_30d") value=fmt_usd(d.total_cost_usd) />
                                    <StatCard label=tr("observability.stat.total_calls") value=d.total_calls.to_string() />
                                    <StatCard label=tr("observability.stat.median_latency") value=fmt_ms(d.median_latency_ms) />
                                    <StatCard label=tr("observability.stat.quality_score") value=format!("{:.0}%", d.quality_score * 100.0) />
                                </div>

                                // Cost by model
                                <Section title=tr("observability.section.cost_by_model")>
                                    <CostBar rows=d.cost_by_model.clone() total=d.total_cost_usd />
                                </Section>

                                // Latency by model
                                <Section title=tr("observability.section.latency_by_model")>
                                    <LatencyTable rows=d.latency_by_model.clone() />
                                </Section>

                                // Cascade efficiency
                                <Section title=tr("observability.section.cascade_efficiency")>
                                    <CascadeDonut
                                        classifier=d.cascade_efficiency.classifier_resolved
                                        sonnet=d.cascade_efficiency.sonnet_escalated
                                        saved_usd=d.cascade_efficiency.saved_usd
                                    />
                                </Section>

                                // Invocation types
                                <Section title=tr("observability.section.invocation_types")>
                                    {if d.invocation_types.is_empty() {
                                        view! { <div class="text-sm italic" style="color:var(--ink-40);">{move || tr("observability.no_data")}</div> }.into_any()
                                    } else {
                                        let max_count = d.invocation_types.iter().map(|i| i.count).max().unwrap_or(1).max(1);
                                        view! {
                                            <div class="space-y-2">
                                                {d.invocation_types.iter().map(|inv| {
                                                    let pct = (inv.count * 100 / max_count).min(100) as u32;
                                                    let label = inv.invocation_type.clone();
                                                    let count = inv.count;
                                                    view! {
                                                        <div class="flex items-center gap-3">
                                                            <div class="w-32 text-xs font-mono font-bold truncate">{label}</div>
                                                            <div class="flex-1 h-5 border border-ink overflow-hidden" style="background:var(--cream-light);">
                                                                <div style=move || format!("width:{}%; height:100%; background:#C0392B;", pct)></div>
                                                            </div>
                                                            <div class="w-8 text-end font-mono text-sm font-bold">{count}</div>
                                                        </div>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        }.into_any()
                                    }}
                                </Section>

                                // Quality signals
                                <Section title=tr("observability.section.quality_signals")>
                                    <QualitySignals signals=d.quality_signals.clone() />
                                </Section>

                                // Recent errors
                                <Section title=tr("observability.section.recent_errors")>
                                    {if d.recent_errors.is_empty() {
                                        view! { <div class="text-sm font-medium" style="color:#1A6B5E;">{move || tr("observability.no_errors")}</div> }.into_any()
                                    } else {
                                        view! {
                                            <div class="overflow-x-auto">
                                                <table class="w-full text-xs border-collapse">
                                                    <thead>
                                                        <tr class="border-b-2 border-ink">
                                                            <th class="text-start font-bold py-1 pe-3 uppercase tracking-wider">{move || tr("observability.col.timestamp")}</th>
                                                            <th class="text-start font-bold py-1 pe-3 uppercase tracking-wider">{move || tr("observability.col.provider")}</th>
                                                            <th class="text-start font-bold py-1 pe-3 uppercase tracking-wider">{move || tr("observability.col.model")}</th>
                                                            <th class="text-start font-bold py-1 pe-3 uppercase tracking-wider">{move || tr("observability.col.type")}</th>
                                                            <th class="text-start font-bold py-1 uppercase tracking-wider">{move || tr("observability.col.error")}</th>
                                                        </tr>
                                                    </thead>
                                                    <tbody>
                                                        {d.recent_errors.iter().map(|e| {
                                                            let ts = e.created_at.clone();
                                                            let prov = e.provider.clone();
                                                            let model = e.model.clone();
                                                            let inv = e.invocation_type.clone();
                                                            let err = e.error.clone();
                                                            view! {
                                                                <tr class="border-b" style="border-color:var(--ink-15);">
                                                                    <td class="py-1.5 pe-3 font-mono whitespace-nowrap">{ts}</td>
                                                                    <td class="py-1.5 pe-3">{prov}</td>
                                                                    <td class="py-1.5 pe-3 font-mono">{model}</td>
                                                                    <td class="py-1.5 pe-3 italic">{inv}</td>
                                                                    <td class="py-1.5 text-brick truncate max-w-[300px]">{err}</td>
                                                                </tr>
                                                            }
                                                        }).collect::<Vec<_>>()}
                                                    </tbody>
                                                </table>
                                            </div>
                                        }.into_any()
                                    }}
                                </Section>
                            </div>
                        }.into_any()
                    }
                }
            }}
        </div>
    }
}
