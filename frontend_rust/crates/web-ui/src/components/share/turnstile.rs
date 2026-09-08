use leptos::prelude::*;

#[derive(Clone, Copy)]
pub struct ShareChallenge {
    pub token: RwSignal<String>,
    pub required: RwSignal<bool>,
    pub reset: RwSignal<u64>,
}

impl ShareChallenge {
    pub fn new() -> Self {
        Self {
            token: RwSignal::new(String::new()),
            required: RwSignal::new(true),
            reset: RwSignal::new(0),
        }
    }

    pub fn blocked(self) -> bool {
        self.required.get() && self.token.get().is_empty()
    }
    pub fn blocked_untracked(self) -> bool {
        self.required.get_untracked() && self.token.get_untracked().is_empty()
    }
    pub fn consume(self) -> Option<String> {
        if !self.required.get_untracked() {
            return None;
        }
        let token = self.token.get_untracked();
        self.token.set(String::new());
        self.reset.update(|n| *n += 1);
        Some(token)
    }
}

#[component]
pub fn Turnstile(challenge: ShareChallenge) -> impl IntoView {
    let auth = expect_context::<RwSignal<String>>();
    let host = NodeRef::<leptos::html::Div>::new();
    let i18n = crate::i18n::use_i18n();
    Effect::new(move |_| {
        let signed_in = !auth.get().is_empty();
        let _ = challenge.reset.get();
        #[cfg(target_arch = "wasm32")]
        if let Some(host) = host.get() {
            let key = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| {
                    d.query_selector("meta[name='turnstile-site-key']")
                        .ok()
                        .flatten()
                })
                .and_then(|el| el.get_attribute("content"))
                .unwrap_or_default();
            challenge.token.set(String::new());
            challenge.required.set(!signed_in && !key.is_empty());
            mount(host.as_ref(), if signed_in { "" } else { &key });
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = signed_in;
    });
    on_cleanup(move || {
        #[cfg(target_arch = "wasm32")]
        if let Some(host) = host.get_untracked() {
            unmount(host.as_ref());
        }
    });
    view! {
        <div node_ref=host data-testid="share-turnstile">
            <input type="hidden" on:input=move |event| challenge.token.set(event_target_value(&event))/>
            <div data-challenge-host></div>
            <p role="status" hidden=move || !challenge.blocked()>{move || i18n.t("share.challengeRequired")}</p>
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
const widgets = new WeakMap();
let loader;
function load() {
  if (window.turnstile) return Promise.resolve(window.turnstile);
  if (!loader) loader = new Promise((resolve, reject) => {
    const script = document.createElement('script');
    script.src = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit';
    script.async = true;
    script.onload = () => resolve(window.turnstile);
    script.onerror = () => { script.remove(); loader = undefined; reject(new Error('challenge unavailable')); };
    document.head.append(script);
  });
  return loader;
}
export function unmount(host) {
  const state = widgets.get(host);
  if (!state) return;
  widgets.delete(host);
  if (state.id !== undefined) window.turnstile?.remove(state.id);
}
export function mount(host, key) {
  unmount(host);
  if (!key) return;
  const state = {};
  widgets.set(host, state);
  const report = token => {
    if (!host.isConnected || widgets.get(host) !== state) return;
    const input = host.querySelector('input');
    input.value = token;
    input.dispatchEvent(new Event('input', { bubbles: true }));
  };
  load().then(api => {
    if (!host.isConnected || widgets.get(host) !== state) return;
    state.id = api.render(host.querySelector('[data-challenge-host]'), {
      sitekey: key,
      callback: report,
      'expired-callback': () => report(''),
      'error-callback': () => report(''),
    });
  }).catch(() => report(''));
}
"#)]
extern "C" {
    fn mount(host: &wasm_bindgen::JsValue, key: &str);
    fn unmount(host: &wasm_bindgen::JsValue);
}
