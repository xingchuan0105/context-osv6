use leptos::prelude::*;

use crate::state::virtual_list::{HeightState, compute_window};

#[component]
pub fn VirtualTextList(
    #[prop(into)] row_heights: Signal<Vec<HeightState>>,
    #[prop(into)] viewport_height_px: Signal<f64>,
    #[prop(into)] scroll_top_px: Signal<f64>,
    overscan: usize,
    children: Children,
) -> impl IntoView {
    let window = Signal::derive(move || {
        compute_window(
            &row_heights.get(),
            scroll_top_px.get(),
            viewport_height_px.get(),
            overscan,
        )
    });

    view! {
        <div
            attr:data-window-start=move || window.get().start_index.to_string()
            attr:data-window-end=move || window.get().end_index.to_string()
        >
            <div style=move || format!("height: {}px;", window.get().top_spacer_px)></div>
            {children()}
            <div style=move || format!("height: {}px;", window.get().bottom_spacer_px)></div>
        </div>
    }
}
