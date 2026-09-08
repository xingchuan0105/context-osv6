use leptos::prelude::*;

#[component]
pub fn NoteEditor(value: RwSignal<String>) -> impl IntoView {
    let host = NodeRef::<leptos::html::Div>::new();
    Effect::new(move |_| {
        let Some(_host) = host.get() else {
            return;
        };
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use wasm_bindgen::JsValue;
            use wasm_bindgen_futures::JsFuture;
            let el = _host;
            let value = value;
            leptos::task::spawn_local(async move {
                let Ok(promise) = js_sys::eval("import('/js/tiptap-editor-bridge.mjs')")
                    .and_then(|v| v.dyn_into::<js_sys::Promise>())
                else {
                    return;
                };
                let Ok(module) = JsFuture::from(promise).await else {
                    return;
                };
                let Ok(init) = js_sys::Reflect::get(&module, &JsValue::from_str("initNoteEditor"))
                else {
                    return;
                };
                let cb = wasm_bindgen::closure::Closure::wrap(Box::new(move |md: JsValue| {
                    if let Some(text) = md.as_string() {
                        value.set(text);
                    }
                }) as Box<dyn FnMut(JsValue)>);
                let args = js_sys::Array::new();
                args.push(&el);
                args.push(&JsValue::from_str(&value.get_untracked()));
                args.push(cb.as_ref());
                let func: js_sys::Function = init.unchecked_into();
                let _ = func.apply(&JsValue::NULL, &args);
                cb.forget();
            });
        }
    });
    view! {
        <div class="note-editor" data-testid="note-editor">
            <div class="note-editor-host" node_ref=host data-testid="note-editor-host"></div>
            <textarea
                class="note-editor-fallback"
                data-testid="note-content-input"
                rows=4
                prop:value=move || value.get()
                on:input=move |ev| value.set(event_target_value(&ev))
            ></textarea>
        </div>
    }
}
