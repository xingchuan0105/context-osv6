//! Shared native presentation. No API, conversation or knowledge state lives here.
use crate::*;

pub const NAV_WIDTH: f32 = 248.;
pub const RAIL_WIDTH: f32 = 60.;
pub const HEADER_HEIGHT: f32 = 52.;
pub const READING_WIDTH: f32 = 760.;

#[derive(Default)]
pub struct Preferences {
    pub collapsed: Option<bool>,
    pub dark: Option<bool>,
    pub error: Option<String>,
}

impl Preferences {
    fn path() -> Option<std::path::PathBuf> {
        std::env::var_os("CONTEXT_OS_CLIENT_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| dirs::data_local_dir().map(|p| p.join("Context-OS Client")))
            .map(|p| p.join("gpui-appearance.json"))
    }

    pub fn load() -> Self {
        let value = Self::path()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        Self {
            collapsed: value.as_ref().and_then(|v| v["collapsed"].as_bool()),
            dark: value.as_ref().and_then(|v| v["dark"].as_bool()),
            error: None,
        }
    }

    pub fn save(&mut self) {
        let result = (|| -> std::io::Result<()> {
            let path = Self::path()
                .ok_or_else(|| std::io::Error::other("appearance directory unavailable"))?;
            std::fs::create_dir_all(path.parent().unwrap())?;
            std::fs::write(
                path,
                serde_json::json!({"collapsed": self.collapsed, "dark": self.dark}).to_string(),
            )
        })();
        self.error = result
            .err()
            .map(|_| "外观偏好未能保存，本次窗口仍可使用。".into());
    }
}

pub fn apply_theme(mode: ThemeMode, window: &mut Window, cx: &mut App) {
    Theme::change(mode, Some(window), cx);
    let theme = Theme::global_mut(cx);
    theme.font_size = px(16.);
    theme.radius = px(8.);
    theme.sheet.margin_top = px(0.);
}

pub fn muted(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(13.))
        .line_height(relative(1.5))
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}

pub fn card(cx: &App) -> Div {
    div()
        .min_w_0()
        .p_4()
        .rounded_xl()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
}

pub fn icon(name: IconName) -> Icon {
    Icon::new(name).size(px(18.))
}

pub fn action(id: impl Into<ElementId>, name: IconName, label: impl Into<SharedString>) -> Button {
    Button::new(id)
        .ghost()
        .rounded(px(8.))
        .icon(icon(name))
        .label(label)
        .text_size(px(13.))
}

pub fn badge(label: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .flex_shrink_0()
        .px_2()
        .py_1()
        .rounded_md()
        .text_size(px(12.))
        .bg(cx.theme().muted)
        .text_color(cx.theme().muted_foreground)
        .child(label.into())
}

/// Renders kit-owned modal layers outside ChatApp's mutable render borrow.
/// Root owns focus trapping, Esc, pointer occlusion and focus return.
pub struct Surface {
    app: Entity<ChatApp>,
    _subscription: Subscription,
}

impl Surface {
    pub fn new(app: Entity<ChatApp>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&app, |_, _, cx| cx.notify());
        Self {
            app,
            _subscription: subscription,
        }
    }
}

impl Render for Surface {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(self.app.clone())
            .children(Root::render_sheet_layer(window, cx))
    }
}
