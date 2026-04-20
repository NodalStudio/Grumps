use leptos::prelude::*;
use crate::api::CalendarItem;
use crate::i18n::tr;

#[component]
pub fn CalItem(item: CalendarItem, compact: bool) -> impl IntoView {
    let color = item.color.clone();
    let title_attr = tr(&item.title);
    let title_body = item.title.clone();
    let source = item.source.clone();

    view! {
        <div
            class="px-1 py-0.5 text-[11px] font-semibold rounded-sm truncate cursor-pointer"
            style:background=color
            style="color: white; border-left: 3px solid rgba(0,0,0,0.3);"
            title=title_attr
        >
            {if compact {
                view! { <span>{move || tr(&title_body)}</span> }.into_any()
            } else {
                let source_str = if source.is_empty() { None } else { Some(format!("({})", source)) };
                view! {
                    <span>
                        {move || tr(&title_body)}
                        {source_str.map(|s| view! { <span class="opacity-70 ml-1">{s}</span> })}
                    </span>
                }.into_any()
            }}
        </div>
    }
}
