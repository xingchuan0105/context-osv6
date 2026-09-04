/// 测试缝：隔离测试用显式 base URL；默认为空字符串（同源 /api/…）。
/// 仅浏览器端读取内存态全局变量，不读取/持久化任何凭据。
#[cfg(target_arch = "wasm32")]
pub fn poc_api_base() -> String {
    web_sys::window()
        .and_then(|window| {
            js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("__POC_CHAT_API_BASE__"))
                .ok()
        })
        .and_then(|value| value.as_string())
        .map(|base| base.trim_end_matches('/').to_string())
        .unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn poc_api_base() -> String {
    String::new()
}
