use crate::api::CalendarItem;
use crate::i18n::tr;
use leptos::prelude::*;

#[component]
pub fn CalItem(item: CalendarItem, compact: bool) -> impl IntoView {
    let color = item.color.clone();
    // Titles run through tr() like every other list in the app: real user
    // titles pass through unchanged, while seed/demo keys (seed.event.*, …)
    // resolve to the active locale.
    let title_attr = item.title.clone();
    let title_body = item.title.clone();
    let source = item.source.clone();

    view! {
        <div
            class="px-1 py-0.5 text-[11px] font-semibold rounded-xs truncate cursor-pointer"
            style:background=color
            style="color: white; border-left: 3px solid rgba(0,0,0,0.3);"
            title=move || tr(&title_attr)
        >
            {if compact {
                let t = title_body.clone();
                view! { <span>{move || tr(&t)}</span> }.into_any()
            } else {
                let t = title_body.clone();
                let src_key = if source.is_empty() { None } else { Some(format!("calendar.source.{}", source)) };
                view! {
                    <span>
                        {move || tr(&t)}
                        {src_key.map(|k| view! { <span class="opacity-70 ms-1">"(" {move || tr(&k)} ")"</span> })}
                    </span>
                }.into_any()
            }}
        </div>
    }
}
