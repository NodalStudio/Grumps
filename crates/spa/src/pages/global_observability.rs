//! Cross-workspace observability dashboard — super admin only.
//! Route: /admin/observability

use leptos::prelude::*;
use leptos_router::components::A;
use crate::auth::use_auth;
use crate::api::{GlobalObservabilityData, GlobalWorkspaceStats, GlobalModelCostAgg, QualitySignalCount, GlobalError};
use crate::components::Icon;

// ── helpers ───────────────────────────────────────────────────────────────────

fn fmt_usd(v: f64) -> String {
    format!("${:.4}", v)
}

fn provider_color(provider: &str) -> &'static str {
    match provider {
        "anthropic" => "#C0392B",
        "gemini"    => "#1A6B5E",
        _           => "#555555",
    }
}

fn signal_label(s: &str) -> (&'static str, &'static str) {
    match s {
        "praise"          => ("Praise",     "#1A6B5E"),
        "thanks"          => ("Thanks",     "#2E8B57"),
        "silence_request" => ("Silence",    "#C0392B"),
        "forget_request"  => ("Forget",     "#E67E22"),
        "correction"      => ("Correction", "#8B0000"),
        "confusion"       => ("Confusion",  "#7D6608"),
        _                 => ("Other",      "#555555"),
    }
}

// ── Hero stat card ────────────────────────────────────────────────────────────

#[component]
fn StatCard(label: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="flex-1 min-w-[160px] border-2 border-strong bg-surface p-4"
             style="box-shadow: 3px 3px 0 var(--border-strong);">
            <div class="font-display text-[2.6rem] font-extrabold leading-none">{value}</div>
            <div class="text-meta mt-1 text-muted">{label}</div>
        </div>
    }
}

// ── Section wrapper ───────────────────────────────────────────────────────────

#[component]
fn Section(title: &'static str, children: Children) -> impl IntoView {
    view! {
        <section class="border-2 border-strong bg-surface p-5 mb-6" style="box-shadow: 3px 3px 0 var(--border-strong);">
            <h2 class="font-display text-xl font-extrabold uppercase tracking-tight mb-4 border-b-2 border-strong pb-2">{title}</h2>
            {children()}
        </section>
    }
}

// ── Cost by model bar ─────────────────────────────────────────────────────────

#[component]
fn ModelCostBar(rows: Vec<GlobalModelCostAgg>, total: f64) -> impl IntoView {
    if rows.is_empty() || total == 0.0 {
        return view! { <div class="text-sm italic text-muted">"No data yet."</div> }.into_any();
    }

    let segments: Vec<_> = rows.iter().map(|r| {
        let pct = if total > 0.0 { (r.cost_usd / total * 100.0) as u32 } else { 0 };
        let color = provider_color(&r.provider);
        (pct, color, r.model.clone(), r.cost_usd, r.call_count)
    }).collect();

    view! {
        <div>
            <div class="flex h-10 border-2 border-strong overflow-hidden mb-3">
                {segments.iter().map(|(pct, color, _m, _c, _n)| {
                    let style = format!("width: {}%; background: {}; flex-shrink:0;", pct, color);
                    view! { <div style=style></div> }
                }).collect::<Vec<_>>()}
            </div>
            <table class="w-full text-sm border-collapse">
                <thead>
                    <tr class="border-b-2 border-strong">
                        <th class="text-left text-meta py-1 pr-4">"Model"</th>
                        <th class="text-right text-meta py-1 px-2">"Calls"</th>
                        <th class="text-right text-meta py-1 pl-2">"Cost"</th>
                    </tr>
                </thead>
                <tbody>
                    {rows.iter().map(|r| {
                        let dot_color = provider_color(&r.provider).to_string();
                        let model = r.model.clone();
                        let calls = r.call_count;
                        let cost = fmt_usd(r.cost_usd);
                        view! {
                            <tr class="border-b border-subtle">
                                <td class="py-1.5 pr-4 flex items-center gap-2">
                                    <span class="inline-block w-3 h-3 border border-strong flex-shrink-0"
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
        return view! { <div class="text-sm italic text-muted">"No workspaces."</div> }.into_any();
    }
    view! {
        <div class="overflow-x-auto">
            <table class="w-full text-sm border-collapse">
                <thead>
                    <tr class="border-b-2 border-strong">
                        <th class="text-left text-meta py-1 pr-4">"Slug"</th>
                        <th class="text-left text-meta py-1 pr-4">"Name"</th>
                        <th class="text-left text-meta py-1 pr-4">"Plan"</th>
                        <th class="text-right text-meta py-1 px-2">"Calls"</th>
                        <th class="text-right text-meta py-1 px-2">"Cost (30j)"</th>
                        <th class="text-right text-meta py-1 px-2">"Quality"</th>
                        <th class="text-right text-meta py-1 pl-2">"Detail"</th>
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
                            <tr class="border-b border-subtle transition-colors hover:bg-hover-tint"
                                style="cursor: pointer;"
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
                                    <span class="text-eyebrow px-1.5 py-0.5 border border-strong bg-surface-raised">{plan}</span>
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
        return view! { <div class="text-sm italic text-muted">"No quality signals yet."</div> }.into_any();
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
                        <div class="flex-1 h-5 border border-strong bg-surface-raised overflow-hidden">
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
        return view! { <div class="text-sm font-medium" style="color:#1A6B5E;">"Aucune erreur. Parfait."</div> }.into_any();
    }
    view! {
        <div class="overflow-x-auto">
            <table class="w-full text-xs border-collapse">
                <thead>
                    <tr class="border-b-2 border-strong">
                        <th class="text-left font-bold py-1 pr-3 uppercase tracking-wider">"Workspace"</th>
                        <th class="text-left font-bold py-1 pr-3 uppercase tracking-wider">"Timestamp"</th>
                        <th class="text-left font-bold py-1 pr-3 uppercase tracking-wider">"Provider"</th>
                        <th class="text-left font-bold py-1 pr-3 uppercase tracking-wider">"Model"</th>
                        <th class="text-left font-bold py-1 uppercase tracking-wider">"Erreur"</th>
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
                            <tr class="border-b border-subtle">
                                <td class="py-1.5 pr-3 font-mono font-bold">{ws}</td>
                                <td class="py-1.5 pr-3 font-mono whitespace-nowrap">{ts}</td>
                                <td class="py-1.5 pr-3">{prov}</td>
                                <td class="py-1.5 pr-3 font-mono">{model}</td>
                                <td class="py-1.5 text-accent truncate max-w-[300px]">{err}</td>
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
    let auth = use_auth();

    let api = auth.api.clone();
    let data = LocalResource::new(move || {
        let api = api.clone();
        async move { api.get_global_observability().await.ok() }
    });

    view! {
        <div class="flex min-h-screen bg-surface-raised">
            // Minimal sidebar for global admin page
            <aside class="w-56 min-w-[220px] flex flex-col border-r-2 border-strong bg-surface-raised">
                <div class="px-5 pt-6 pb-5 border-b-2 border-strong">
                    <h1 class="font-display text-xl font-extrabold uppercase tracking-tight">
                        "GRUMPS"<span class="text-accent">"."</span>
                    </h1>
                    <p class="text-eyebrow mt-0.5 text-muted">
                        "Super Admin"
                    </p>
                </div>
                <nav class="py-3 flex-1">
                    <div class="px-5 pt-4 pb-1.5 text-eyebrow text-muted">"Admin"</div>
                    <A href="/admin/observability"
                       attr:class="flex items-center gap-2.5 px-5 py-2 text-sm font-medium border-l-[3px] border-accent text-primary">
                        <Icon name="globe" class="size-4 flex-shrink-0"/>
                        "Observabilité globale"
                    </A>
                    <A href="/dashboard"
                       attr:class="flex items-center gap-2.5 px-5 py-2 text-sm font-medium border-l-[3px] border-transparent hover:bg-hover-tint text-secondary">
                        <span class="w-[18px] text-center text-body">"⊞"</span>
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
                            <div class="font-display text-xl animate-pulse text-muted">"Loading…"</div>
                        }.into_any(),
                        Some(None) => view! {
                            <div class="border-2 border-strong bg-surface p-6" style="box-shadow:3px 3px 0 var(--border-strong);">
                                <div class="font-display text-xl font-extrabold text-accent">"Accès refusé ou erreur."</div>
                                <p class="text-sm mt-2 text-secondary">"Cette page est réservée aux super admins."</p>
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
                                            "Observabilité globale"
                                            <span class="text-accent">"."</span>
                                        </h1>
                                        <p class="text-sm font-medium uppercase tracking-widest mt-1 text-muted">
                                            {d.generated_at.clone()}
                                        </p>
                                    </div>

                                    // Hero stats
                                    <div class="flex flex-wrap gap-4 mb-8">
                                        <StatCard label="Workspaces" value=d.workspaces_count.to_string() />
                                        <StatCard label="Coût total (30j)" value=fmt_usd(d.total_cost_usd) />
                                        <StatCard label="Total appels" value=d.total_calls.to_string() />
                                        <StatCard label="Score qualité moy." value=format!("{:.0}%", avg_quality * 100.0) />
                                    </div>

                                    // Cost by model
                                    <Section title="Coût agrégé par modèle (30j)">
                                        <ModelCostBar rows=d.cost_by_model.clone() total=d.total_cost_usd />
                                    </Section>

                                    // Top workspaces by cost
                                    <Section title="Top workspaces par coût">
                                        <WorkspacesTable rows=d.by_workspace.clone() />
                                    </Section>

                                    // Quality signals
                                    <Section title="Signaux qualité agrégés (30j)">
                                        <QualitySignals signals=d.quality_signals.clone() />
                                    </Section>

                                    // Recent errors
                                    <Section title="Erreurs récentes (toutes workspaces)">
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
