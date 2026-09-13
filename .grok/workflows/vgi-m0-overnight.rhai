let meta = #{
    name: "vgi-m0-overnight",
    description: "Unattended VGI M0 regenerate: corpus + token-batch + zvec-spike into /home/chuan/vgi-rs",
    when_to_use: "Overnight VGI M0, goal-mode regenerate from docs/vgi, no human at the keyboard",
    phases: [
        #{ title: "Discover", detail: "toolchain, disk, parquet shards" },
        #{ title: "Scaffold", detail: "independent vgi-rs workspace" },
        #{ title: "Corpus", detail: "fingerprint + ingest from corpus.md" },
        #{ title: "Tokens", detail: "windows + token_count from token-batch.md" },
        #{ title: "Zvec", detail: "spike from zvec-spike.md" },
        #{ title: "Verify", detail: "anchors, cargo test, fixture fp, spike json" },
        #{ title: "Ingest", detail: "optional full BrowseComp-Plus ingest" },
        #{ title: "Evidence", detail: "adversarial check of M0 gates" },
        #{ title: "Report", detail: "overnight-report.md" },
    ],
};

fn trimmed(s) {
    if type_of(s) == "string" {
        s.trim();
        s
    } else {
        ""
    }
}

let spec = "/home/chuan/context-osv6/docs/vgi";
let code = "/home/chuan/vgi-rs";
let shards = "/mnt/c/Users/xingc/Documents/Codex/repository-tree/.eval/hard/challenge20-v1/shards";
let product = "/home/chuan/context-osv6";

let bounds = "HARD BOUNDS. Spec dir: ";
bounds += spec;
bounds += ". Code dir: ";
bounds += code;
bounds += ". Shards: ";
bounds += shards;
bounds += ". Set CARGO_BUILD_JOBS=2. Write jobs=2 in ";
bounds += code;
bounds += "/.cargo/config.toml. Do not cargo-build avrag-rs or anything under ";
bounds += product;
bounds += "/avrag-rs. Do not HTTP-embed, Milvus, MCP, sandbox, docker-compose, git push, or add crates to the product workspace. Do not invent chunk_id. If you change behavior, edit the spec doc first. Log misreads in ";
bounds += spec;
bounds += "/log.md. git init ";
bounds += code;
bounds += " and commit locally there. Read files with tools before writing code. ";

let discover_schema = #{
    "type": "object",
    "required": ["rust_ok", "disk_gb", "corpus_present", "notes"],
    "properties": #{
        "rust_ok": #{ "type": "boolean" },
        "disk_gb": #{ "type": "number" },
        "corpus_present": #{ "type": "boolean" },
        "notes": #{ "type": "string" },
    },
};

let verify_schema = #{
    "type": "object",
    "required": ["ok", "gaps"],
    "properties": #{
        "ok": #{ "type": "boolean" },
        "gaps": #{ "type": "array", "items": #{ "type": "string" } },
        "notes": #{ "type": "string" },
    },
};

let evidence_schema = #{
    "type": "object",
    "required": ["real", "reason", "evidence"],
    "properties": #{
        "real": #{ "type": "boolean" },
        "reason": #{ "type": "string" },
        "evidence": #{ "type": "string" },
    },
};

phase("Discover");
let disc_p = bounds;
disc_p += "Run terminal commands. Check: rustc --version, cargo --version, df -BG /home/chuan, ls the shards directory. ";
disc_p += "Set rust_ok true only if both rustc and cargo run. disk_gb is available gigabytes on /home/chuan (number). ";
disc_p += "corpus_present true only if train-00000-of-00007.parquet exists in the shards dir. notes = one short paragraph of facts.";
let disc = agent(disc_p, #{
    label: "discover",
    capability_mode: "execute",
    output_schema: discover_schema,
});
let rust_ok = false;
let disk_gb = 0;
let corpus_present = false;
if disc != () && disc.success && disc.output != () {
    if disc.output.rust_ok != () { rust_ok = disc.output.rust_ok; }
    if disc.output.disk_gb != () { disk_gb = disc.output.disk_gb; }
    if disc.output.corpus_present != () { corpus_present = disc.output.corpus_present; }
}
log("discover rust_ok=" + rust_ok.to_string() + " disk_gb=" + disk_gb.to_string() + " corpus=" + corpus_present.to_string());
if rust_ok == false {
    let p = write_scratch_file("report.md", "M0 overnight aborted: rustc/cargo missing.");
    complete(#{ status: "failed", reason: "no rust toolchain", path: p });
}

phase("Scaffold");
let sc_p = bounds;
sc_p += "Create an independent Cargo workspace at ";
sc_p += code;
sc_p += " if it does not already exist. Crates: vgi-core (lib), vgi-ingest (lib), vgi (bin with subcommands ingest, tokens, spike-zvec, status). ";
sc_p += "edition 2021. README one paragraph pointing at ";
sc_p += spec;
sc_p += "/README.md. .gitignore for /target and /.vgi/jobs/*.tmp. git init and one commit. Do not copy avrag-rs crates. If the tree already exists, inspect and only fill missing pieces.";
let sc = agent(sc_p, #{ label: "scaffold", capability_mode: "all" });
if sc == () || sc.success == false {
    log("scaffold agent failed; continuing so later modules can still try");
}

phase("Corpus");
let c_p = bounds;
c_p += "Read ALL of ";
c_p += spec;
c_p += "/corpus.md with the read_file tool. Implement fingerprint + ingest exactly. SHA-256 UTF-8, NUL-separated rows, sort by doc_id bytes, payload = revision + LF + joined rows. ";
c_p += "Worked example MUST produce corpus_fp 5f76e86df681122f018c02203b95a682d692f37e5c88de93ba3643c786d500b2. ";
c_p += "Add a unit test that encodes that anchor. Command: vgi ingest --fixture (two docs) and vgi ingest --uri <parquet-dir> --revision b27b02bc3e45511b8b82a13e6f90ce761df726f6 --expected-n 100195. ";
c_p += "SQLite schema exactly as the doc. Do not create passages. Do not implement token_count fill here. cargo test -p vgi-core -p vgi-ingest with CARGO_BUILD_JOBS=2. Commit in ";
c_p += code;
c_p += ".";
let c_impl = agent(c_p, #{ label: "impl-corpus", capability_mode: "all" });

phase("Tokens");
let t_p = bounds;
t_p += "Read ALL of ";
t_p += spec;
t_p += "/token-batch.md. Implement windows(n_tokens): LIMIT=7372, OVERLAP=256, STRIDE=7116, CAP=64. Anchors: 0->[0,0); 100; 7372; 7373->[0,7372)+[7116,7373); 8000; 455680 last [448308,455680) truncated=false; 455681 truncated=true. ";
t_p += "fill_token_counts uses HuggingFace tokenizers crate with BAAI/bge-m3. Unit tests for the integer window table (no network). cargo test CARGO_BUILD_JOBS=2. Commit in ";
t_p += code;
t_p += ".";
let t_impl = agent(t_p, #{ label: "impl-tokens", capability_mode: "all" });

phase("Zvec");
let z_p = bounds;
z_p += "Read ALL of ";
z_p += spec;
z_p += "/zvec-spike.md. Implement vgi spike-zvec: n=10000, dim=1024, fp16, HNSW cosine, seed=0 fake vectors, 32 queries top-10, write ";
z_p += code;
z_p += "/.vgi/jobs/zvec-spike.json with ok, n, dim, dtype, p95_query_ms, rss_bytes, backend, error. Prefer crate zvec-rust feature bundled. If link/API fails, same SpikeReport via hnsw_rs and backend=hnsw_rs. Do not ingest the BrowseComp corpus into the ANN. cargo test CARGO_BUILD_JOBS=2. Run the spike once if compile succeeds. Commit.";
let z_impl = agent(z_p, #{ label: "impl-zvec", capability_mode: "all" });

phase("Verify");
let v_p = bounds;
v_p += "Reproduce M0 gates with commands, do not trust prior agents. Required: (1) python3 ";
v_p += spec;
v_p += "/anchors/test_anchors.py exits 0. (2) cd ";
v_p += code;
v_p += " && CARGO_BUILD_JOBS=2 cargo test exits 0. (3) fixture ingest twice, both corpus_fp 5f76e86df681122f018c02203b95a682d692f37e5c88de93ba3643c786d500b2. (4) ";
v_p += code;
v_p += "/.vgi/jobs/zvec-spike.json exists, ok=true, n=10000, dim=1024, dtype=fp16. ";
v_p += "Set ok=true only if all four hold. gaps = failing checks as short strings. If cargo test fails, you MAY fix compile errors that do not change the spec, then re-run. Do not weaken anchors.";
let ver = agent(v_p, #{
    label: "verify",
    capability_mode: "all",
    output_schema: verify_schema,
});
let verify_ok = false;
let gaps = [];
if ver != () && ver.success && ver.output != () {
    if ver.output.ok != () { verify_ok = ver.output.ok; }
    if ver.output.gaps != () { gaps = ver.output.gaps; }
}
if verify_ok == false {
    log("verify red; one repair round");
    let r_p = bounds;
    r_p += "Previous verify reported gaps as JSON: ";
    r_p += json_encode(gaps);
    r_p += " . Read ";
    r_p += spec;
    r_p += "/corpus.md ";
    r_p += spec;
    r_p += "/token-batch.md ";
    r_p += spec;
    r_p += "/zvec-spike.md and the failing tests. Fix implementation to match the spec. Re-run cargo test and the fixture fingerprint. Do not change expected hashes.";
    let repair = agent(r_p, #{ label: "repair", capability_mode: "all" });
    let ver2 = agent(v_p, #{
        label: "verify-2",
        capability_mode: "all",
        output_schema: verify_schema,
    });
    if ver2 != () && ver2.success && ver2.output != () {
        if ver2.output.ok != () { verify_ok = ver2.output.ok; }
        if ver2.output.gaps != () { gaps = ver2.output.gaps; }
    }
}

phase("Ingest");
let did_full = false;
if corpus_present == true && disk_gb >= 10 && verify_ok == true {
    let i_p = bounds;
    i_p += "Full ingest. URI ";
    i_p += shards;
    i_p += " revision b27b02bc3e45511b8b82a13e6f90ce761df726f6 expected_n 100195 into ";
    i_p += code;
    i_p += "/.vgi/ . If n_docs != 100195, stop, write measured N to ";
    i_p += spec;
    i_p += "/log.md and do not pretend success. If n_docs matches, ingest a second time and compare corpus_fp. Then vgi tokens to fill token_count (BAAI/bge-m3). If tokenizer download fails, leave token_count null and record it. Do not embed. Stop if df avail would fall below 8G.";
    let ing = agent(i_p, #{ label: "full-ingest", capability_mode: "all" });
    if ing != () && ing.success { did_full = true; }
} else {
    log("skip full ingest (corpus_present=" + corpus_present.to_string() + " disk_gb=" + disk_gb.to_string() + " verify_ok=" + verify_ok.to_string() + ")");
}

phase("Evidence");
let e_p = "Adversarial evidence review of VGI M0. Use tools. Claim: M0 regenerate succeeded. Set real=true ONLY if you independently ran or read the artifacts: (1) python3 ";
e_p += spec;
e_p += "/anchors/test_anchors.py ok, (2) cargo test ok in ";
e_p += code;
e_p += ", (3) fixture corpus_fp 5f76e86df681122f018c02203b95a682d692f37e5c88de93ba3643c786d500b2, (4) zvec-spike.json ok=true n=10000 dim=1024 fp16. ";
e_p += "If any artifact is missing, real=false. reason = one paragraph. evidence = commands and file paths you actually inspected, not the implementer's word. Full-corpus ingest is optional and must not decide real.";
let ev = agent(e_p, #{
    label: "evidence",
    capability_mode: "execute",
    output_schema: evidence_schema,
});
let real = false;
let ev_reason = "";
let ev_evidence = "";
if ev != () && ev.success && ev.output != () {
    if ev.output.real != () { real = ev.output.real; }
    if ev.output.reason != () { ev_reason = ev.output.reason; }
    if ev.output.evidence != () { ev_evidence = ev.output.evidence; }
}

phase("Report");
let rp = bounds;
rp += "Write ";
rp += spec;
rp += "/overnight-report.md in Chinese, short. Sections: 结果 (done|partial|failed), 证据路径, 跳过项, 全库 ingest 是否跑, cargo/zvec 后端, 明天建议. ";
rp += "Evidence-review real=";
rp += real.to_string();
rp += " reason JSON ";
rp += json_encode(ev_reason);
rp += " evidence JSON ";
rp += json_encode(ev_evidence);
rp += " verify_ok=";
rp += verify_ok.to_string();
rp += " gaps JSON ";
rp += json_encode(gaps);
rp += " did_full=";
rp += did_full.to_string();
rp += " . Also append a dated note to ";
rp += spec;
rp += "/log.md. Commit the report in the product repo if you have git write; commit code in ";
rp += code;
rp += " only.";
let report_agent = agent(rp, #{ label: "report", capability_mode: "all" });

let status = "partial";
if real == true { status = "done"; }
if verify_ok == false && real == false { status = "failed"; }

let report_md = "# VGI M0 overnight\n\nstatus: " + status + "\n\n";
report_md += ev_reason;
report_md += "\n\n";
report_md += ev_evidence;
let path = write_scratch_file("report.md", report_md);
complete(#{
    status: status,
    real: real,
    verify_ok: verify_ok,
    full_ingest: did_full,
    path: path,
    code: code,
});
