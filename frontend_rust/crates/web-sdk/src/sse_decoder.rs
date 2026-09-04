//! 增量 SSE wire decoder：从任意字节分块还原 `contracts::chat::ChatEvent`。
//!
//! 平台中立：不依赖 DOM、Leptos、路由或浏览器 storage。输入是 HTTP 响应体
//! 的任意字节分块，输出是严格反序列化的 `ChatEvent`；任何坏帧都是 typed
//! error，绝不静默跳过。
//!
//! Wire 格式（与后端 `transport-http/src/handlers/chat.rs` 对齐）：
//! `event: <snake_case 名>\ndata: <ChatEvent JSON 单行>\n\n`，KeepAlive 为
//! `: <text>` 注释行。SSE `event:` 字段属于 framing，反序列化前必须合入
//! JSON 对象（data JSON 内的同名字段被其覆盖）。

use crate::transport::TransportError;
use contracts::chat::ChatEvent;
use futures_util::Stream;

/// 增量 SSE decoder。按 `push` 喂入任意对齐/非对齐字节分块，`finish` 在 EOF
/// 时冲刷尾部没有空行收尾的最后一个事件。
pub struct SseDecoder {
    /// 尚未构成完整 UTF-8 序列的尾部字节
    pending_bytes: Vec<u8>,
    /// 已解码但尚未出现换行符的文本前缀
    line_buf: String,
    /// 当前累积事件的 `event:` 名
    event_name: Option<String>,
    /// 当前累积事件的 `data:` 行
    data_lines: Vec<String>,
    /// 本事件是否见过 `data:` 字段（区分缺失与空值）
    has_data_field: bool,
}

impl SseDecoder {
    pub fn new() -> Self {
        Self {
            pending_bytes: Vec::new(),
            line_buf: String::new(),
            event_name: None,
            data_lines: Vec::new(),
            has_data_field: false,
        }
    }

    /// 喂入一块响应体字节，返回本块内完整解码出的事件。
    /// 任一坏帧立即返回 typed error（调用方应终止该流）。
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<ChatEvent>, TransportError> {
        self.pending_bytes.extend_from_slice(bytes);
        let text = self.take_valid_utf8()?;
        self.line_buf.push_str(&text);

        let mut events = Vec::new();
        while let Some(newline) = self.line_buf.find('\n') {
            let mut line = self.line_buf.drain(..=newline).collect::<String>();
            line.pop(); // '\n'
            if line.ends_with('\r') {
                line.pop();
            }
            if let Some(event) = self.process_line(&line)? {
                events.push(event);
            }
        }
        Ok(events)
    }

    /// EOF 冲刷：处理残余字节与最后一个没有空行收尾的事件。
    pub fn finish(&mut self) -> Result<Vec<ChatEvent>, TransportError> {
        if !self.pending_bytes.is_empty() {
            let rest = std::mem::take(&mut self.pending_bytes);
            let text = String::from_utf8(rest).map_err(|error| {
                TransportError::Framing(format!("invalid UTF-8 at end of stream: {error}"))
            })?;
            self.line_buf.push_str(&text);
        }

        let mut events = Vec::new();
        if !self.line_buf.is_empty() {
            let mut line = std::mem::take(&mut self.line_buf);
            if line.ends_with('\r') {
                line.pop();
            }
            if let Some(event) = self.process_line(&line)? {
                events.push(event);
            }
        }
        if self.event_name.is_some() || self.has_data_field {
            if let Some(event) = self.dispatch_event()? {
                events.push(event);
            }
        }
        Ok(events)
    }

    /// 取走 pending_bytes 中最长的合法 UTF-8 前缀；不完整尾部序列留待下一块。
    fn take_valid_utf8(&mut self) -> Result<String, TransportError> {
        match std::str::from_utf8(&self.pending_bytes) {
            Ok(valid) => {
                let text = valid.to_string();
                self.pending_bytes.clear();
                Ok(text)
            }
            Err(error) => {
                let valid_up_to = error.valid_up_to();
                if valid_up_to == 0 && error.error_len().is_some() {
                    return Err(TransportError::Framing(format!(
                        "invalid UTF-8 in stream: {error}"
                    )));
                }
                let valid = self.pending_bytes[..valid_up_to].to_vec();
                self.pending_bytes.drain(..valid_up_to);
                if error.error_len().is_some() {
                    return Err(TransportError::Framing(format!(
                        "invalid UTF-8 in stream: {error}"
                    )));
                }
                // error_len() == None：尾部是未完成的 UTF-8 序列，跨 chunk 正常
                String::from_utf8(valid)
                    .map_err(|e| TransportError::Framing(format!("invalid UTF-8: {e}")))
            }
        }
    }

    /// 处理一行。返回 Some(event) 表示空行触发了一次 dispatch。
    fn process_line(&mut self, line: &str) -> Result<Option<ChatEvent>, TransportError> {
        if line.is_empty() {
            return self.dispatch_event();
        }
        if line.starts_with(':') {
            // comment / keepalive：不得触发 dispatch，也不进入事件数据
            return Ok(None);
        }

        let (field, value) = match line.find(':') {
            Some(colon) => {
                let mut value = &line[colon + 1..];
                if value.starts_with(' ') {
                    value = &value[1..];
                }
                (&line[..colon], value)
            }
            None => (line, ""),
        };

        match field {
            "event" => self.event_name = Some(value.to_string()),
            "data" => {
                self.has_data_field = true;
                self.data_lines.push(value.to_string());
            }
            // id / retry / 未知字段：SSE spec 允许存在，不作为事件数据
            _ => {}
        }
        Ok(None)
    }

    fn dispatch_event(&mut self) -> Result<Option<ChatEvent>, TransportError> {
        let event_name = self.event_name.take();
        let data_lines = std::mem::take(&mut self.data_lines);
        let has_data_field = self.has_data_field;
        self.has_data_field = false;

        if event_name.is_none() && !has_data_field {
            // 只有注释/空行，没有累积任何事件内容
            return Ok(None);
        }

        let name = event_name
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| TransportError::Framing("SSE event is missing `event:` field".into()))?;
        if !has_data_field {
            return Err(TransportError::Framing(format!(
                "SSE event `{name}` is missing `data:` field"
            )));
        }

        let data = data_lines.join("\n");
        let mut value: serde_json::Value = serde_json::from_str(&data).map_err(|error| {
            TransportError::Framing(format!("SSE event `{name}` has non-JSON data: {error}"))
        })?;
        let object = value.as_object_mut().ok_or_else(|| {
            TransportError::Framing(format!("SSE event `{name}` data is not a JSON object"))
        })?;
        // framing 层的 event 名合入对象，覆盖 data 内同名字段
        object.insert("event".to_string(), serde_json::Value::String(name));

        serde_json::from_value::<ChatEvent>(value)
            .map(Some)
            .map_err(TransportError::Serialization)
    }
}

impl Default for SseDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// 把任意「字节分块流」适配为「ChatEvent 流」：decoder 的平台中立消费回路，
/// 浏览器 Fetch 与测试共用一个实现。字节流中途失败或坏帧产出且仅产出一次
/// typed error 并终止（坏流不伪装成正常结束）。
pub fn events_from_byte_stream<S>(
    byte_stream: S,
) -> impl Stream<Item = Result<ChatEvent, TransportError>>
where
    S: Stream<Item = Result<Vec<u8>, TransportError>>,
{
    use futures_util::StreamExt;

    async_stream::stream! {
        let mut decoder = SseDecoder::new();
        let mut upstream = Box::pin(byte_stream);
        while let Some(chunk) = upstream.next().await {
            match chunk {
                Ok(bytes) => match decoder.push(&bytes) {
                    Ok(events) => {
                        for event in events {
                            yield Ok(event);
                        }
                    }
                    Err(error) => {
                        yield Err(error);
                        return;
                    }
                },
                Err(error) => {
                    yield Err(error);
                    return;
                }
            }
        }
        match decoder.finish() {
            Ok(events) => {
                for event in events {
                    yield Ok(event);
                }
            }
            Err(error) => yield Err(error),
        }
    }
}
