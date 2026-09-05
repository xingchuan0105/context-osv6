/// Product capability tags on the chat wire (`rag` / `search`).
/// Order is stable: rag then search. Unknown values are ignored.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Rag,
    Search,
}

impl Capability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rag => "rag",
            Self::Search => "search",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "rag" => Some(Self::Rag),
            "search" => Some(Self::Search),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Rag => "知识库",
            Self::Search => "网络搜索",
        }
    }
}

/// Stable order: rag then search; dedupe; ignore unknown.
pub fn normalize_capabilities<I, S>(input: I) -> Vec<Capability>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut rag = false;
    let mut search = false;
    for item in input {
        match Capability::parse(item.as_ref()) {
            Some(Capability::Rag) => rag = true,
            Some(Capability::Search) => search = true,
            None => {}
        }
    }
    let mut out = Vec::new();
    if rag {
        out.push(Capability::Rag);
    }
    if search {
        out.push(Capability::Search);
    }
    out
}

pub fn toggle_capability(current: &[Capability], cap: Capability) -> Vec<Capability> {
    if current.contains(&cap) {
        current.iter().copied().filter(|item| *item != cap).collect()
    } else {
        let mut next: Vec<&str> = current.iter().map(|item| item.as_str()).collect();
        next.push(cap.as_str());
        normalize_capabilities(next)
    }
}

pub fn derive_agent_type_label(capabilities: &[Capability]) -> &'static str {
    match (
        capabilities.contains(&Capability::Rag),
        capabilities.contains(&Capability::Search),
    ) {
        (true, true) => "rag+search",
        (true, false) => "rag",
        (false, true) => "search",
        (false, false) => "chat",
    }
}

pub fn capabilities_to_wire(capabilities: &[Capability]) -> Vec<String> {
    capabilities
        .iter()
        .map(|cap| cap.as_str().to_string())
        .collect()
}

/// Auto-attach rag when ready session files appear; strip auto rag when they
/// drop to zero. Manual chip edits are left alone.
pub fn reconcile_session_rag(
    current: &[Capability],
    manual: bool,
    ready_count: usize,
) -> Vec<Capability> {
    let has_rag = current.contains(&Capability::Rag);
    if ready_count == 0 {
        if has_rag && !manual {
            return current
                .iter()
                .copied()
                .filter(|cap| *cap != Capability::Rag)
                .collect();
        }
        return current.to_vec();
    }
    if !has_rag && !manual {
        return toggle_capability(current, Capability::Rag);
    }
    current.to_vec()
}

/// Personal-chat mode line. No right-rail copy: hint session files instead.
pub fn mode_line(capabilities: &[Capability], rag_available: bool) -> &'static str {
    if !rag_available && capabilities.is_empty() {
        return "未添加会话文件：添加文件后可检索本会话。";
    }
    match (
        capabilities.contains(&Capability::Rag),
        capabilities.contains(&Capability::Search),
    ) {
        (true, true) => "知识库 + 网络搜索：回答检索已选文档与网页。",
        (true, false) => "知识库：回答检索已选文档。",
        (false, true) => "网络搜索：回答会检索网页。",
        (false, false) => "聊天：回答不检索文档与网络。",
    }
}
