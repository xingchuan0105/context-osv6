//! Default scenario catalogue.
use super::types::Scenario;

pub fn default_scenarios() -> Vec<Scenario> {
    vec![
        // --- Chat ---
        Scenario {
            name: "chat_simple_cn",
            mode: "chat",
            query: "你好",
            history: vec![],
            user_preferences: None,
            search_results: vec![],
            rag_chunks: vec![],
        },
        Scenario {
            name: "chat_medium_cn",
            mode: "chat",
            query: "请总结量子计算的基本原理和应用场景",
            history: vec![
                ("user", "什么是量子比特？"),
                ("assistant", "量子比特是量子计算的基本单位..."),
            ],
            user_preferences: Some(serde_json::json!({"style": "concise"})),
            search_results: vec![],
            rag_chunks: vec![],
        },
        Scenario {
            name: "chat_complex_en",
            mode: "chat",
            query: "Compare the memory safety guarantees of Rust, Swift, and ATS, focusing on how each language handles dangling pointers and use-after-free. Provide concrete code examples.",
            history: vec![],
            user_preferences: None,
            search_results: vec![],
            rag_chunks: vec![],
        },
        // --- Search ---
        Scenario {
            name: "search_simple",
            mode: "search",
            query: "Rust async runtime comparison",
            history: vec![],
            user_preferences: None,
            search_results: vec![
                (
                    "Tokio vs async-std",
                    "Tokio is the most widely used async runtime in Rust...",
                ),
                (
                    "Rust Async Book",
                    "The async book covers the fundamentals of async/await in Rust...",
                ),
                (
                    "Comparing Rust Runtimes",
                    "A detailed benchmark comparing Tokio, async-std, and smol...",
                ),
            ],
            rag_chunks: vec![],
        },
        Scenario {
            name: "search_complex",
            mode: "search",
            query: "2026年最新的大语言模型推理优化技术有哪些？对比 DeepSeek、Qwen 和 Gemini 的推理架构差异",
            history: vec![],
            user_preferences: Some(serde_json::json!({"style": "detailed", "language": "zh"})),
            search_results: vec![
                (
                    "DeepSeek V4 推理优化",
                    "DeepSeek V4 introduces speculative decoding with tree attention...",
                ),
                (
                    "Qwen3 技术报告",
                    "Qwen3 employs a mixture-of-experts architecture with 128 experts...",
                ),
                (
                    "Gemini 3.5 Flash 架构",
                    "Gemini 3.5 Flash uses a novel attention mechanism called multi-query...",
                ),
                (
                    "LLM 推理优化综述 2026",
                    "A comprehensive survey covering quantization, pruning, distillation...",
                ),
                (
                    "Speculative Decoding Survey",
                    "Speculative decoding has become the standard for latency reduction...",
                ),
            ],
            rag_chunks: vec![],
        },
        // --- RAG ---
        Scenario {
            name: "rag_simple",
            mode: "rag",
            query: "这份合同中的违约责任条款是什么？",
            history: vec![],
            user_preferences: None,
            search_results: vec![],
            rag_chunks: vec![
                "第七条 违约责任\n7.1 任何一方违反本合同约定，应当向守约方承担违约责任。",
                "7.2 违约金计算方式：按合同总金额的10%计算。",
                "7.3 因不可抗力导致无法履行合同的，双方均不承担违约责任。",
            ],
        },
        Scenario {
            name: "rag_complex",
            mode: "rag",
            query: "分析这份技术方案中数据库架构的风险点，并提出优化建议。重点关注高可用性、数据一致性和扩展性。",
            history: vec![
                ("user", "请先概述整体架构"),
                (
                    "assistant",
                    "该方案采用微服务架构，数据库层使用 PostgreSQL 主从复制...",
                ),
            ],
            user_preferences: Some(
                serde_json::json!({"style": "structured", "expertise": "senior engineer"}),
            ),
            search_results: vec![],
            rag_chunks: vec![
                "数据库架构设计\n本文档描述了一套基于 PostgreSQL 的高可用数据库架构。",
                "3.1 主从复制\n采用流复制（Streaming Replication）机制，主节点写入，从节点读取。",
                "3.2 故障切换\n使用 Patroni + etcd 实现自动故障检测和主从切换，RTO < 30s。",
                "3.3 数据一致性\n同步复制模式确保主从数据强一致，但会增加写入延迟约 5-10ms。",
                "3.4 扩展性\n通过 Citus 实现分片扩展，支持水平扩展到 100+ 节点。",
                "3.5 风险分析\n主要风险包括：脑裂场景、网络分区时的数据不一致、以及分片键选择不当导致的查询热点。",
                "4.1 优化建议\n建议引入读写分离中间件（如 PgPool-II），并考虑使用逻辑复制替代物理复制以提高灵活性。",
            ],
        },
    ]
}

// ---------------------------------------------------------------------------
// Simulation engine
// ---------------------------------------------------------------------------
