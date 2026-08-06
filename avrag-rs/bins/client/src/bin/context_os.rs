//! context-os — thin CLI over the same local/cloud HTTP MCP surface.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use context_os::config::{self, ClientConfig};
use context_os::discover;
use context_os::mcp_client::{McpClient, tool_data};
use context_os::mime;
use serde_json::json;

fn print_usage() {
    eprintln!(
        "\
context-os — thin client for local/cloud Context-OS (same auth as MCP)

USAGE:
  context-os status
  context-os ingest [--workspace ID] [--no-wait] [--timeout SECS] <file>
  context-os ask [--workspace ID] <query...>
  context-os sources [--workspace ID]
  context-os share …          (refused: user session only)
  context-os --help

ENV:
  CONTEXT_OS_API_BASE / AVRAG_PUBLIC_BASE_URL   (default http://127.0.0.1:18080)
  CONTEXT_OS_API_KEY / CONTEXT_OS_WORKSPACE_API_KEY
  CONTEXT_OS_WORKSPACE_ID

FLAGS (global, before or after subcommand):
  --base URL          Override API base
  --key KEY           Override API key
  --workspace ID      Workspace UUID

Examples:
  context-os status
  context-os ingest --workspace $WS ./notes.pdf
  context-os ask --workspace $WS \"Summarize the docs\"
  context-os sources --workspace $WS

stdio MCP for coding agents: context-os-mcp
Docs: docs/desktop/LOCAL-CLIENT-MCP-CLI-AGENT-ACCESS.md
"
    );
}

#[derive(Debug, Default)]
struct GlobalOpts {
    base: Option<String>,
    key: Option<String>,
    workspace: Option<String>,
}

#[derive(Debug)]
enum Command {
    Status,
    Ingest {
        path: PathBuf,
        wait: bool,
        timeout_secs: u64,
    },
    Ask {
        query: String,
    },
    Sources,
    Share,
    Help,
}

fn parse_args(args: &[String]) -> Result<(GlobalOpts, Command)> {
    let mut opts = GlobalOpts::default();
    let mut rest: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => return Ok((opts, Command::Help)),
            "--base" => {
                i += 1;
                let v = args
                    .get(i)
                    .context("--base requires a URL")?
                    .clone();
                opts.base = Some(v);
            }
            "--key" => {
                i += 1;
                let v = args.get(i).context("--key requires a value")?.clone();
                opts.key = Some(v);
            }
            "--workspace" | "-w" => {
                i += 1;
                let v = args
                    .get(i)
                    .context("--workspace requires a UUID")?
                    .clone();
                opts.workspace = Some(v);
            }
            other if other.starts_with('-') => {
                // Leave flag for subcommand parsers by collecting after we know command.
                rest.push(other.to_string());
            }
            other => rest.push(other.to_string()),
        }
        i += 1;
    }

    if rest.is_empty() {
        return Ok((opts, Command::Help));
    }

    let cmd = rest[0].as_str();
    let sub = &rest[1..];

    match cmd {
        "status" | "check" => Ok((opts, Command::Status)),
        "help" => Ok((opts, Command::Help)),
        "share" => Ok((opts, Command::Share)),
        "sources" | "list-sources" => Ok((opts, Command::Sources)),
        "ask" | "query" | "rag" => {
            let mut query_parts: Vec<String> = Vec::new();
            let mut j = 0;
            while j < sub.len() {
                match sub[j].as_str() {
                    "--workspace" | "-w" => {
                        j += 1;
                        opts.workspace = Some(
                            sub.get(j)
                                .context("ask --workspace requires UUID")?
                                .clone(),
                        );
                    }
                    "--base" => {
                        j += 1;
                        opts.base = Some(sub.get(j).context("ask --base requires URL")?.clone());
                    }
                    "--key" => {
                        j += 1;
                        opts.key = Some(sub.get(j).context("ask --key requires value")?.clone());
                    }
                    part => query_parts.push(part.to_string()),
                }
                j += 1;
            }
            let query = query_parts.join(" ").trim().to_string();
            if query.is_empty() {
                bail!("ask requires a query string");
            }
            Ok((opts, Command::Ask { query }))
        }
        "ingest" | "upload" => {
            let mut wait = true;
            let mut timeout_secs = 180u64;
            let mut path: Option<PathBuf> = None;
            let mut j = 0;
            while j < sub.len() {
                match sub[j].as_str() {
                    "--no-wait" => wait = false,
                    "--wait" => wait = true,
                    "--timeout" => {
                        j += 1;
                        timeout_secs = sub
                            .get(j)
                            .context("--timeout requires seconds")?
                            .parse()
                            .context("--timeout must be an integer")?;
                    }
                    "--workspace" | "-w" => {
                        j += 1;
                        opts.workspace = Some(
                            sub.get(j)
                                .context("ingest --workspace requires UUID")?
                                .clone(),
                        );
                    }
                    "--base" => {
                        j += 1;
                        opts.base =
                            Some(sub.get(j).context("ingest --base requires URL")?.clone());
                    }
                    "--key" => {
                        j += 1;
                        opts.key =
                            Some(sub.get(j).context("ingest --key requires value")?.clone());
                    }
                    part if part.starts_with('-') => {
                        bail!("unknown ingest flag: {part}");
                    }
                    part => {
                        if path.is_some() {
                            bail!("ingest accepts a single file path");
                        }
                        path = Some(PathBuf::from(part));
                    }
                }
                j += 1;
            }
            let path = path.context("ingest requires a file path")?;
            Ok((
                opts,
                Command::Ingest {
                    path,
                    wait,
                    timeout_secs,
                },
            ))
        }
        other => bail!("unknown command `{other}` (try context-os --help)"),
    }
}

fn build_config(opts: &GlobalOpts) -> Result<ClientConfig> {
    Ok(ClientConfig::from_env()?
        .with_api_base(opts.base.clone())
        .with_api_key(opts.key.clone())
        .with_workspace_id(opts.workspace.clone()))
}

async fn cmd_ingest(client: &McpClient, path: &PathBuf, wait: bool, timeout_secs: u64) -> Result<()> {
    let workspace_id = client.config().require_workspace_id()?.to_string();
    let meta = tokio::fs::metadata(path)
        .await
        .with_context(|| format!("stat {}", path.display()))?;
    if !meta.is_file() {
        bail!("{} is not a regular file", path.display());
    }
    let file_size = meta.len();
    if file_size == 0 {
        bail!("file is empty");
    }
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("upload.bin")
        .to_string();
    let mime_type = mime::guess_mime(&filename);

    eprintln!(
        "context-os: create_upload workspace={workspace_id} file={filename} size={file_size} mime={mime_type}"
    );

    let created = client
        .tools_call(
            "workspace.create_upload",
            json!({
                "workspace_id": workspace_id,
                "filename": filename,
                "mime_type": mime_type,
                "file_size": file_size,
            }),
        )
        .await?;
    let data = tool_data(&created);
    let document_id = data
        .get("document_id")
        .and_then(|v| v.as_str())
        .context("create_upload missing document_id")?
        .to_string();
    let upload_url = data
        .get("upload_url")
        .and_then(|v| v.as_str())
        .context("create_upload missing upload_url")?;
    let put_url = client.resolve_url(upload_url);

    let bytes = tokio::fs::read(path)
        .await
        .with_context(|| format!("read {}", path.display()))?;
    eprintln!("context-os: PUT {} ({} bytes)", put_url, bytes.len());
    client.put_bytes(&put_url, bytes).await?;

    eprintln!("context-os: complete_upload document_id={document_id}");
    client
        .tools_call(
            "workspace.complete_upload",
            json!({
                "workspace_id": workspace_id,
                "document_id": document_id,
            }),
        )
        .await?;

    if !wait {
        println!(
            "{}",
            json!({
                "ok": true,
                "document_id": document_id,
                "status": "submitted",
                "workspace_id": workspace_id,
            })
        );
        return Ok(());
    }

    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
    let mut last = String::new();
    loop {
        let st = client
            .tools_call(
                "workspace.document_status",
                json!({
                    "workspace_id": workspace_id,
                    "document_id": document_id,
                }),
            )
            .await?;
        let data = tool_data(&st);
        let status = data
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        if status != last {
            eprintln!("context-os: document_status → {status}");
            last = status.clone();
        }
        match status.as_str() {
            "completed" | "ready" | "indexed" => {
                println!(
                    "{}",
                    json!({
                        "ok": true,
                        "document_id": document_id,
                        "status": status,
                        "workspace_id": workspace_id,
                        "data": data,
                    })
                );
                return Ok(());
            }
            "failed" | "error" => {
                bail!("ingest failed for document_id={document_id}: {data}");
            }
            _ => {}
        }
        if tokio::time::Instant::now() >= deadline {
            bail!(
                "timed out after {timeout_secs}s waiting for document {document_id} (last status: {status})"
            );
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn cmd_ask(client: &McpClient, query: &str) -> Result<()> {
    let workspace_id = client.config().require_workspace_id()?.to_string();
    eprintln!("context-os: rag_query workspace={workspace_id}");
    let result = client
        .tools_call(
            "workspace.rag_query",
            json!({
                "workspace_id": workspace_id,
                "query": query,
            }),
        )
        .await?;
    let data = tool_data(&result);

    // Prefer human-readable answer fields when present.
    let answer = data
        .get("answer")
        .and_then(|v| v.as_str())
        .or_else(|| data.get("content").and_then(|v| v.as_str()))
        .or_else(|| data.pointer("/message/content").and_then(|v| v.as_str()));

    if let Some(text) = answer {
        println!("{text}");
        // Also dump structured JSON on stderr for scripts that want both.
        eprintln!("---");
        eprintln!("{}", serde_json::to_string_pretty(data)?);
    } else {
        println!("{}", serde_json::to_string_pretty(data)?);
    }
    Ok(())
}

async fn cmd_sources(client: &McpClient) -> Result<()> {
    let workspace_id = client.config().require_workspace_id()?.to_string();
    let result = client
        .tools_call(
            "workspace.list_sources",
            json!({ "workspace_id": workspace_id }),
        )
        .await?;
    let data = tool_data(&result);
    println!("{}", serde_json::to_string_pretty(data)?);
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let (opts, command) = match parse_args(&args) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("context-os: {e}");
            eprintln!("try: context-os --help");
            return ExitCode::from(2);
        }
    };

    if matches!(command, Command::Help) {
        print_usage();
        return ExitCode::SUCCESS;
    }

    if matches!(command, Command::Share) {
        eprintln!("context-os: {}", config::share_forbidden_message());
        return ExitCode::from(4);
    }

    let cfg = match build_config(&opts) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("context-os: {e}");
            return ExitCode::from(2);
        }
    };

    match command {
        Command::Status => match discover::run_check(&cfg).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(code) => ExitCode::from(code),
        },
        Command::Ingest {
            path,
            wait,
            timeout_secs,
        } => {
            let client = match McpClient::new(cfg) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("context-os: {e}");
                    return ExitCode::FAILURE;
                }
            };
            match cmd_ingest(&client, &path, wait, timeout_secs).await {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("context-os: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Ask { query } => {
            let client = match McpClient::new(cfg) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("context-os: {e}");
                    return ExitCode::FAILURE;
                }
            };
            match cmd_ask(&client, &query).await {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("context-os: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Sources => {
            let client = match McpClient::new(cfg) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("context-os: {e}");
                    return ExitCode::FAILURE;
                }
            };
            match cmd_sources(&client).await {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("context-os: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Share | Command::Help => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_status() {
        let (opts, cmd) = parse_args(&["status".into()]).unwrap();
        assert!(matches!(cmd, Command::Status));
        assert!(opts.workspace.is_none());
    }

    #[test]
    fn parse_ingest_with_flags() {
        let (opts, cmd) = parse_args(&[
            "ingest".into(),
            "--workspace".into(),
            "11111111-1111-1111-1111-111111111111".into(),
            "--no-wait".into(),
            "./a.pdf".into(),
        ])
        .unwrap();
        assert_eq!(
            opts.workspace.as_deref(),
            Some("11111111-1111-1111-1111-111111111111")
        );
        match cmd {
            Command::Ingest { path, wait, .. } => {
                assert_eq!(path, PathBuf::from("./a.pdf"));
                assert!(!wait);
            }
            _ => panic!("expected ingest"),
        }
    }

    #[test]
    fn parse_ask_query_words() {
        let (_, cmd) = parse_args(&[
            "ask".into(),
            "--workspace".into(),
            "ws".into(),
            "what".into(),
            "is".into(),
            "this?".into(),
        ])
        .unwrap();
        match cmd {
            Command::Ask { query } => assert_eq!(query, "what is this?"),
            _ => panic!("expected ask"),
        }
    }

    #[test]
    fn parse_share() {
        let (_, cmd) = parse_args(&["share".into(), "enable".into()]).unwrap();
        assert!(matches!(cmd, Command::Share));
    }
}
