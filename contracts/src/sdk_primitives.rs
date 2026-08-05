//! SaC SDK 原语注册表 —— 沙箱方法单一事实源(D10, 2026-08-01)。
//!
//! 每个沙箱原语(id + capability 归属 + docstring + host handler 绑定)只在这里定义一次:
//! - `agent-loop` 的 `sdk_gate` 从本表派生 capability allowlist(替代三组硬编码常量)
//! - `code-interpreter` 从本表 codegen Python shim 方法与 docstring
//! - `rag-core` 的 host dispatch 按 `handler` 键分派实现
//!
//! 新增一个原语 = 本表一行 + handler 实现 + 提示词;不再碰多处常量。
//! 本文件只放**纯数据**;handler 实现与 fn 指针在 rag-core(见 `rag-core/src/runtime/`)。

/// 原语的 capability 归属(位标志,可组合):Base 恒开,Rag/Search 按产品 capability 挂载。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SdkCapability(u8);

impl SdkCapability {
    /// 纯 chat 也开放(沙箱基座):跨块存储 + 轻记忆 + 本地工具。
    pub const BASE: Self = Self(1);
    /// 知识库检索(rag capability)。
    pub const RAG: Self = Self(2);
    /// 联网(search capability;`dense` 允许混合检索)。
    pub const SEARCH: Self = Self(4);
    /// 混合检索(dense 在 rag 与 search 都开放)。
    pub const RAG_SEARCH: Self = Self(2 | 4);

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl std::ops::BitOr for SdkCapability {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// 一个沙箱原语的声明式定义。
#[derive(Debug, Clone, Copy)]
pub struct SdkPrimitive {
    /// 沙箱方法名(模型可见)。
    pub id: &'static str,
    pub capability: SdkCapability,
    /// Python shim 的方法 docstring(模型/开发者可见的教学文案)。
    pub docstring: &'static str,
    /// rag-core host dispatch 的 handler 键。
    pub handler: &'static str,
    /// Python shim 方法签名(不含 self 前缀),如 `query`。
    pub py_sig: &'static str,
    /// Python shim payload 表达式(传给 `_rpc(method, payload)`),如 `{"query": query}`。
    pub py_payload: &'static str,
    /// 返回取值路径(如 `["chunks"]`);空 = 原样返回。
    pub py_return: &'static str,
}

/// 全部 SaC SDK 原语(单一事实源)。
pub const SDK_PRIMITIVES: &[SdkPrimitive] = &[
    // ── Base(纯 chat 恒开)──────────────────────────────────────────────
    SdkPrimitive {
        id: "save",
        capability: SdkCapability::BASE,
        docstring: "Persist JSON-serializable data under a relative path (cross-block / cross-turn).",
        handler: "storage_save",
        py_sig: "self, path, data",
        py_payload: "{\"path\": path, \"data\": data}",
        py_return: "",
    },
    SdkPrimitive {
        id: "load",
        capability: SdkCapability::BASE,
        docstring: "Load data previously saved with save(path, data).",
        handler: "storage_load",
        py_sig: "self, path",
        py_payload: "{\"path\": path}",
        py_return: "[\"data\"]",
    },
    SdkPrimitive {
        id: "history",
        capability: SdkCapability::BASE,
        docstring: "Load prior conversation turns (not full chat dump into tokens).",
        handler: "memory_history",
        py_sig: "self, limit=20, query=\"\", scope=\"workspace\"",
        py_payload: "{\"limit\": limit, \"query\": query, \"scope\": scope}",
        py_return: "",
    },
    SdkPrimitive {
        id: "user_profile",
        capability: SdkCapability::BASE,
        docstring: "Load structured user profile preferences.",
        handler: "memory_user_profile",
        py_sig: "self",
        py_payload: "{}",
        py_return: "",
    },
    SdkPrimitive {
        id: "user_context",
        capability: SdkCapability::BASE,
        docstring: "Local clock / 'today' / IP-city context for time- and location-dependent facts.",
        handler: "base_user_context",
        py_sig: "self",
        py_payload: "{}",
        py_return: "",
    },
    SdkPrimitive {
        id: "calculator",
        capability: SdkCapability::BASE,
        docstring: "Evaluate a mathematical expression exactly.",
        handler: "base_calculator",
        py_sig: "self, expression",
        py_payload: "{\"expression\": expression}",
        py_return: "",
    },
    SdkPrimitive {
        id: "weather_query",
        capability: SdkCapability::BASE,
        docstring: "Current weather for a location (QWeather 和风; OpenWeather fallback).",
        handler: "base_weather_query",
        py_sig: "self, city=None, lat=None, lon=None",
        py_payload: "{\"city\": city, \"lat\": lat, \"lon\": lon}",
        py_return: "",
    },
    // ── Rag(知识库检索)─────────────────────────────────────────────────
    SdkPrimitive {
        id: "dense",
        capability: SdkCapability::RAG_SEARCH,
        docstring: "Semantic retrieval (host may expand via entity graph and return a fused chunk list). \
            topk is fixed by the host — only pass query.",
        handler: "retrieval_dense",
        py_sig: "self, query",
        py_payload: "{\"query\": query}",
        py_return: "[\"chunks\"]",
    },
    SdkPrimitive {
        id: "lexical",
        capability: SdkCapability::RAG,
        docstring: "BM25/keyword retrieval. Returns a chunk list (no graph side-car in product default).",
        handler: "retrieval_lexical",
        py_sig: "self, query",
        py_payload: "{\"query\": query}",
        py_return: "[\"chunks\"]",
    },
    SdkPrimitive {
        id: "grep",
        capability: SdkCapability::RAG,
        docstring: "Line-level locate (coding-agent grep). Returns full payload: \
            total_hits / returned / truncated / hits[{doc_id, line, text, before, after}]. \
            total_hits is exact (host-counted) — use it; do not re-count or dedupe in code.",
        handler: "retrieval_grep",
        py_sig: "self, pattern, doc_ids=None, regex=False, context=0, max_hits=50",
        py_payload: "{\"pattern\": pattern, \"regex\": regex, \"context\": context, \"max_hits\": max_hits, **({\"doc_ids\": doc_ids} if doc_ids is not None else {})}",
        py_return: "",
    },
    SdkPrimitive {
        id: "doc_profile",
        capability: SdkCapability::RAG,
        docstring: "Document profile (title/author/genre/era/language) + section structure.",
        handler: "retrieval_doc_profile",
        py_sig: "self, doc_ids=None, fields=None",
        py_payload: "{**({\"doc_ids\": doc_ids} if doc_ids is not None else {}), **({\"fields\": fields} if fields else {})}",
        py_return: "[\"chunks\"]",
    },
    SdkPrimitive {
        id: "doc_summary",
        capability: SdkCapability::RAG,
        docstring: "Whole-document or per-section overview (not verbatim evidence).",
        handler: "retrieval_doc_summary",
        py_sig: "self, level=\"doc\", doc_ids=None",
        py_payload: "{\"level\": level, **({\"doc_ids\": doc_ids} if doc_ids is not None else {})}",
        py_return: "[\"chunks\"]",
    },
    SdkPrimitive {
        id: "struct_catalog",
        capability: SdkCapability::RAG,
        docstring: "List table relations in per-doc DuckDB struct stores (name/headers/n_rows/\
            sample_rows/caption/unit/confidence). Empty relations = no struct store (「无表格」).",
        handler: "struct_catalog",
        py_sig: "self, doc_ids=None",
        py_payload: "{\"doc_ids\": doc_ids} if doc_ids is not None else {}",
        py_return: "",
    },
    SdkPrimitive {
        id: "struct_query",
        capability: SdkCapability::RAG,
        docstring: "Run one restricted SELECT against the struct store. COUNT/filter/order are \
            engine-exact. Returns {ok, columns, rows, row_count, evidence} or {ok:false, error}.",
        handler: "struct_query",
        py_sig: "self, sql, doc_ids=None",
        py_payload: "{\"sql\": sql, **({\"doc_ids\": doc_ids} if doc_ids is not None else {})}",
        py_return: "",
    },
    // ── Search(联网)────────────────────────────────────────────────────
    SdkPrimitive {
        id: "web",
        capability: SdkCapability::SEARCH,
        docstring: "Web search (SaC). Fan-out multiple queries in one code block when needed.",
        handler: "search_web",
        py_sig: "self, query",
        py_payload: "{\"query\": query}",
        py_return: "",
    },
    SdkPrimitive {
        id: "fetch",
        capability: SdkCapability::SEARCH,
        docstring: "Fetch one URL and extract readable text.",
        handler: "search_fetch",
        py_sig: "self, url",
        py_payload: "{\"url\": url}",
        py_return: "",
    },
];

/// capability 归属查询。
pub fn primitive(id: &str) -> Option<&'static SdkPrimitive> {
    SDK_PRIMITIVES.iter().find(|p| p.id == id)
}

/// 按 capability 收集 id(包含匹配:如 Rag 会同时拿到 dense(RAG|SEARCH))。
pub fn ids_for(cap: SdkCapability) -> Vec<&'static str> {
    SDK_PRIMITIVES
        .iter()
        .filter(|p| p.capability.contains(cap))
        .map(|p| p.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_unique_ids_and_handlers() {
        let mut ids = std::collections::HashSet::new();
        let mut handlers = std::collections::HashSet::new();
        for p in SDK_PRIMITIVES {
            assert!(ids.insert(p.id), "duplicate id: {}", p.id);
            assert!(handlers.insert(p.handler), "duplicate handler: {}", p.handler);
            assert!(!p.docstring.is_empty());
        }
    }

    #[test]
    fn base_always_open_and_rag_search_partitioned() {
        let base = ids_for(SdkCapability::BASE);
        let rag = ids_for(SdkCapability::RAG);
        let search = ids_for(SdkCapability::SEARCH);
        assert!(base.contains(&"save") && base.contains(&"user_context"));
        assert!(rag.contains(&"dense") && rag.contains(&"struct_query"));
        assert!(search.contains(&"web") && search.contains(&"dense"));
        // Base 原语不进入检索面;rag 检索原语不进入 search 面(除 dense 混合)
        for b in &base {
            assert!(!rag.contains(b) && !search.contains(b), "{b} 不应属于检索面");
        }
        assert!(!search.contains(&"grep"));
        assert!(!rag.contains(&"web"));
    }
}
