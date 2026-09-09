use leptos::prelude::*;

#[component]
pub fn NoteEditor(value: RwSignal<String>) -> impl IntoView {
    let host = NodeRef::<leptos::html::Div>::new();
    let ready = RwSignal::new(false);
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
            let lifecycle = StoredValue::new_local(None::<(js_sys::Object, wasm_bindgen::closure::Closure<dyn FnMut(JsValue)>)>);
            on_cleanup(move || {
                lifecycle.try_update_value(|handle| {
                    if let Some((editor, _callback)) = handle.take() {
                        if let Ok(destroy) = js_sys::Reflect::get(&editor, &JsValue::from_str("destroy")).and_then(|v| v.dyn_into::<js_sys::Function>()) {
                            let _ = destroy.call0(&editor);
                        }
                    }
                });
            });
            leptos::task::spawn_local(async move {
                let Ok(promise) = js_sys::eval("import('/js/tiptap-editor-bridge.mjs')")
                    .and_then(|v| v.dyn_into::<js_sys::Promise>())
                else {
                    return;
                };
                let Ok(module) = JsFuture::from(promise).await else {
                    return;
                };
                if lifecycle.is_disposed() { return; }
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
                if let Ok(editor) = func.apply(&JsValue::NULL, &args).and_then(|v| v.dyn_into::<js_sys::Object>()) {
                    lifecycle.set_value(Some((editor, cb)));
                    ready.set(true);
                }
            });
        }
    });
    view! {
        <div class="note-editor" data-testid="note-editor" data-ready=move || ready.get().to_string()>
            <div class="note-editor-host" node_ref=host data-testid="note-editor-host" hidden=move || !ready.get()></div>
            <textarea
                class="note-editor-fallback"
                data-testid="note-content-input"
                hidden=move || ready.get()
                rows=4
                prop:value=move || value.get()
                on:input=move |ev| value.set(event_target_value(&ev))
            ></textarea>
        </div>
    }
}
