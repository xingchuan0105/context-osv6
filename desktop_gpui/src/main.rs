use desktop_gpui::reduce_fixture_json_lines;
use gpui::prelude::*;
use gpui::{App, Application, Bounds, Context, Render, Window, WindowOptions, div, px, rgb, size};
use web_sdk::TurnStatus;

const FIXTURE: &str = include_str!("../../frontend_rust/tests/fixtures/stream-normal-long.json");

struct ChatPreview {
    answer: String,
    status: String,
}

impl Render for ChatPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .bg(rgb(0x0f172a))
            .text_color(rgb(0xe2e8f0))
            .size_full()
            .p_4()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x94a3b8))
                    .child(format!("status: {}", self.status)),
            )
            .child(div().text_sm().child(self.answer.clone()))
    }
}

fn main() {
    let state = reduce_fixture_json_lines(FIXTURE).expect("fixture reduces");
    let status = match &state.status {
        TurnStatus::Done => "done".to_string(),
        TurnStatus::Streaming => "streaming".to_string(),
        TurnStatus::Idle => "idle".to_string(),
        TurnStatus::Cancelled => "cancelled".to_string(),
        TurnStatus::Error { code, .. } => format!("error:{code}"),
    };
    let answer = state.answer_text;

    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(720.), px(520.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            move |_, cx| {
                cx.new(|_| ChatPreview {
                    answer: answer.clone(),
                    status: status.clone(),
                })
            },
        )
        .expect("open window");
    });
}
