# Regeneration log

Record misread prose and failed anchors here. Point at the module doc, not the generated file.

- 2026-09-14 `zvec-spike.md`: `Collection::create_and_open` rejects an already-created directory (`path exists, create expects a path that does not exist`). First spike run created the dir then failed; fallback `hnsw_rs` wrote a valid report. Regenerated spike after passing a non-existent path.
- 2026-09-14 `zvec-spike.md`: `SearchQuery::new(&[f32])` passes `size_of_val` bytes; against a VectorFp16 field zvec counted 1024 f32 as 2048 fp16 dims. Query path now sets packed fp16 bytes via the C API.
- 2026-09-14 `token-batch.md`: `ureq` could not reach HuggingFace (`Network is unreachable`) while `curl` through the WSL proxy could. Tokenizer download falls back to `curl`. Official `tokenizer.json` sha256 `21106b6d7dab2952c1d496fb21d5dc9db75c28ed361a05f5020bbba27810dd08`.
