# 交接:ingestion/triplet 三模型基准 —— markitdown 保留决策 + 稳定 runner(2026-08-28)

## 一句话状态

ingestion/triplet 基准(qwen3.7-flash / qwen3.8-flash 同 key 同 URL 走 DashScope,deepseek-v4-flash 走 Ollama cloud)三次起跑失败根因全部查明并修复(两次 PATH、一次漏设 env);解析管线经核实后**用户拍板保留现状(markitdown 继续解析 txt/md/code),不改**;稳定 runner(直跑已编译测试二进制、参数化换 LLM 源、status 文件 10s 轮询)已就绪于 `avrag-rs/crates/app/tests/e2e_output/triplet_benchmark_20260828_b/run_benchmark.sh`。

## 1. 基准对象与指标

- 驱动脚本:`avrag-rs/scripts/benchmark_triplet_models.sh`;底层测试:`triplet_benchmark_huawei_ipd`(`crates/app/tests/product_e2e/llm_real/rag_quality_prod.rs:3019`,默认 `#[ignore]`)。
- 单篇语料 `huawei_ipd_370_activities.txt`,强制重灌库,产出一行 JSON(`TRIPLET_BENCHMARK_RESULT=`):`ingest_secs / chunk_count / entity_count / relation_count / graph_passage_count / graph_degrade_count / recall_at_15 / faithfulness / diagnostic_label / answer_preview`。
- 脚本选项:`BENCHMARK_MODELS`(格式 `provider:model:budget`,provider 白名单 dashscope|gemini|deepseek,base URL 写死)、`RESULTS_DIR`(默认 `/tmp/triplet_benchmark_<ts>`,**WSL 重启会丢**,本轮改到仓内 e2e_output)、`MERGE_SUMMARY`、各 provider key。
- 本轮的口径差异:两槽位(INGESTION_LLM_* 与 TRIPLET_LLM_*)同步换成被测模型,summary 与 triplet 都是被测变量;原脚本只换 TRIPLET 槽。

## 2. 两个模型的历史成绩(背景)

- full-149 lead_workers 评测(qwen3.7-flash=检索道,deepseek-v4-flash=合成+judge)产出目录:`avrag-rs/crates/app/tests/e2e_output/rag_eval_v2/v2_<ts>/`;文字纪事:`docs/engineering/2026-08-16-retrieve-split-event-log-handoff.md`。
- 趋势:08-11 全 deepseek 基线 126 → 133 → 127(facets)→ 135(grep regex 回退)→ **08-17 最高 141(v2_20260817-062941,recall@k .9385)**。
- ingestion 侧(triplet benchmark)无存活历史产出:默认 RESULTS_DIR 在 /tmp 已被清,且默认横扫名单里没有 qwen3.7-flash——这两个模型此前没跑过该基准。

## 3. 本日运行史与三次失败根因(postmortem:_a 目录)

`avrag-rs/crates/app/tests/e2e_output/triplet_benchmark_20260828_a/`(保留作 postmortem):

1. **10:00 exit 127**:runner 用 `wsl.exe -e bash <脚本>`(非登录 shell)起,PATH 无 `~/.cargo/bin` → `cargo: command not found`。
2. **10:16 秒挂 exit 101**:runner 只报了 `TRIPLET_LLM_MODEL`,漏设测试断言必需的 `TRIPLET_BENCHMARK_MODEL`(及 `_PROVIDER`)→ 测试入口 panic。已补。
3. **10:17 leg1 卡 12 分钟**:测试停在 `[smoke_v5] uploading ...(timeout=360s)`,0 条 LLM 调用;排查链:PG 无 advisory 锁 → 进程 0 CPU、futex 等子进程 avrag-worker → `avrag_rs_e2e_smoke.ingestion_tasks` 里任务每 ~90s 重试、attempt 4/5 → `last_error`:**`markitdown spawn failed (bin "markitdown"): No such file or directory`**。根因还是 PATH:非登录 shell 缺 `~/.local/bin`(pipx 装的 markitdown)。worker 解析第一步 ENOENT → 重试至 dead letter。
   - 修复:runner PATH 双补 `export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"`。
   - 教训已入 memory(wsl-toolchain-github-ssh-workaround):非登录 shell PATH 双补,且 wsl.exe 包装层 exit 0 会误报,必须看日志。

## 4. 解析管线核实结论(用户拍板:保留现状,不改)

核实事实(2026-08-28,工作树 = master 64c02b11):

- 现行路由(`crates/ingestion/src/parser/router/mod.rs`):PDF→liteparse(扫描兜底 PaddleOCR)、Office/ODF/RTF/EPUB/CSV→anydoc 子进程、**txt/md/rst/tsv/json/toml/yaml/yml/html/htm + 代码扩展名→markitdown 子进程**、standalone 图片→PaddleOCR。
- **anydoc(firecrawl-anydoc 0.1.6)不支持 txt**:实测 `anydoc-extract <txt>` → `UnsupportedError: unsupported input`。Format 枚举只有 doc/docx/odt/pdf/ppt/pptx/rtf/epub/xlsx/ods/odp/csv。所以"换成 anydoc"路线对纯文本不可行。
- markitdown 路径的真实分工:子进程只做"文件 → markdown"一步(对 txt≈透传);Heading/Paragraph 切块、行号定位(`MD_LINE_*`)、chunker 全是原生 Rust(`blocks_from_markdown`)。下游三个去处——PG `rag_text_chunks`(纯文本 chunk)、embedding(bge-m3,chunk 文本)、LLM(窗口文本做 summary/triplet)——**消费的都是纯文本 chunk,不依赖任何外部解析器**。
- **决策:不改。** markitdown 保留为文本/代码长尾解析器;子进程失败语义 hard-fail 不降级,属预期。
- 运维面事实:markitdown/anydoc-extract 均为 pipx 类用户级安装(`~/.local/bin`),**任何 spawn 它们的进程(worker、e2e)PATH 必须含 `~/.local/bin`**,否则即现本日第 3 次失败的形态(upload 卡 360s + ingestion_tasks 反复重试)。

## 5. 稳定 runner 设计(_b 目录,换 LLM 源零重编)

`avrag-rs/crates/app/tests/e2e_output/triplet_benchmark_20260828_b/run_benchmark.sh`:

- **不调 cargo**:直接运行已编译测试二进制(`target/debug/deps/product_e2e-<hash>`,运行时 `ls -t` 解析最新),环境不变即零重编;代码真变了才需手动 `cargo test -p app --test product_e2e --features product-e2e --no-run` 一次。
- **腿=参数**:`LEGS` 数组每行 `name|model|base_url|key_env_name|api_style`,换模型/换源只改数组,不动代码。
- **status.txt 事件流**:每条腿 start/end/exit/wall 逐行追加,外部 10s 轮询读它即可,不必盯整日志。
- 每腿结果仍提取 `TRIPLET_BENCHMARK_RESULT` 追加 `summary.jsonl`(leg/exit/wall_secs + 原始指标)。
- 既有口径:INGESTION+TRIPLET 双槽位同步换;`INGESTION_TRIPLET_TOKEN_BUDGET=3000`;timeout 180s;PATH 双补。
- **ingest 等待超时旋钮**:`RAG_SMOKE_INGEST_TIMEOUT_SECS`(`fixtures/smoke_v5_corpus.rs:27`,>0 直接覆盖;默认 120×3=360s——triplet 开启自动×3 仍不够,首轮 qwen3.7-flash ingest >360s 被 `wait_for_ingestion` 掐断,exit 101 无结果行)。runner 已设 900s。同样是纯 env,零重编。

## 6. 已知注意事项(解读结果时带星号)

- **Ollama 腿 thinking 关不掉**:客户端 openai 路径对非 deepseek/wafer 域名发顶层 `enable_thinking:false`,ollama.com 收下但忽略(实测);`reasoning_effort:"none"` 实测可关,但客户端不发该字段。故 deepseek-v4-flash(Ollama)在 thinking 开启下跑,速度与 token 数不可与 qwen(thinking off)直接比。
- 黄金集题号跨 run shuffle;本基准单题(PAC-05),无此问题,但跨 run 对比一律按 query 文本。
- `avrag-test-pg-*` 容器与 e2e PG(avrag_rs_e2e_smoke)按惯例不动;本基准只读。

## 7. 首轮三模型成绩(2026-08-28,_b 目录,RETRIEVAL_BACKEND=milvus)

| 腿 | 端点 | leg wall | **ingest 墙钟**(upload→completed) | PAC-05 | label |
|---|---|---|---|---|---|
| qwen3.7-flash | DashScope 同 key/URL,dashscope_responses,thinking off | 375s | **342.0s** | 答案正确带引用 | PASS |
| qwen3.8-flash | 同上 | 305s | **215.6s** | 答案正确带引用 | PASS |
| deepseek-v4-flash | Ollama cloud `/v1`,openai 风格,**thinking 开** | 215s | **186.6s** | 答案正确带引用 | PASS |

解读注意:
- recall@15=0 / faithfulness=0 三腿一致——golden 期望块与新灌语料的映射失配(分数器工件),**不区分模型**,以答案与 label 为准;要修需重算 golden 映射(未动)。
- Ollama 腿 thinking 未关(§6),186.6s 是 thinking 开的成绩,qwen 两腿是 thinking off——跨家对比带此星号。
- 速度排序:qwen3.8-flash(215.6s)< deepseek-v4-flash-ollama(186.6s 最快,thinking 开)< qwen3.7-flash(342.0s)。ingest 墙钟含 markitdown 解析、窗口 PS+triplet、embedding、图写入全程。

## 8. 本日改动的代码(测试侧,未提交)

- `crates/app/tests/product_e2e/fixtures/smoke_v5_corpus.rs`:`SmokeV5CorpusState.last_ingest_wall_secs`(serde default)——冷灌库 upload→completed 的墙钟在 fixture 现场计时。**原因:T7 窗口管线退役了 `documents`/`document_parse_runs` 行写入(两表恒空,任务停在 processing、完成状态走别的通道),基准原有的三条取数 SQL 全部过时,`query_document_ingest_duration_secs` 必然 "no rows"。**
- `crates/app/tests/product_e2e/llm_real/rag_quality_prod.rs`(`triplet_benchmark_huawei_ipd`):改读 fixture 计时;删除 chunk_count/entity/relation/graph 计数与末尾 graph 断言(同因,数据源不存在);保留 PAC-05 recall/faithfulness/label/answer(API 层,与存储后端无关)。
- **未动**:产品 ingestion 管线、markitdown 路由(用户拍板保留现状)。
- 产出物:`_a/`(四次失败 postmortem)、`_b/`(成绩 summary.jsonl + 三份 leg 日志 + status.txt + run_benchmark.sh)。

## 9. 遗留

- [ ] golden 期望块映射失配导致 recall/faith 恒 0,要修需按新灌语料重算 golden(未动)。
- [ ] 若要把双槽位同步换 + ollama case + 仓内 RESULTS_DIR 回填 `scripts/benchmark_triplet_models.sh`,约 15 行(待拍板)。
- [ ] Ollama 关 thinking 需客户端 openai 路径支持 `reasoning_effort`(3 行,待拍板)。
- [ ] 本日测试侧两文件改动待本地提交。
