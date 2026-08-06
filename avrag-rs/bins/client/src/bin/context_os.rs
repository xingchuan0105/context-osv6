//! context-os — thin CLI over the same local/cloud HTTP MCP surface.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use context_os::config::ClientConfig;
use context_os::discover;
use context_os::mcp_client::{McpClient, tool_data};
use context_os::mime;
use serde_json::{Value, json};

fn print_usage() {
    eprintln!(
        "\
context-os — thin client for local/cloud Context-OS (same auth as MCP)

USAGE:
  context-os status
  context-os auth login --email E --password P
  context-os auth mint [--ttl MINUTES]
  context-os auth whoami
  context-os workspace create --name NAME [--description D]
  context-os workspace list
  context-os ingest [--workspace ID] [--no-wait] [--timeout SECS] <file>
  context-os ask [--workspace ID] <query...>
  context-os sources [--workspace ID]
  context-os share enable [--workspace ID] [--role viewer] [--expires-in SECS]
  context-os share status [--workspace ID]
  context-os share configure [--workspace ID] [--access-level link|public|private] …
  context-os share revoke [--workspace ID] --token TOKEN
  context-os share quota
  context-os --help

ENV:
  CONTEXT_OS_API_BASE / AVRAG_PUBLIC_BASE_URL   (default http://127.0.0.1:18080)
  CONTEXT_OS_API_KEY / CONTEXT_OS_WORKSPACE_API_KEY   (index/query automation)
  CONTEXT_OS_USER_TOKEN / CONTEXT_OS_AGENT_TOKEN      (user JWT; create workspace + share)
  CONTEXT_OS_WORKSPACE_ID

FLAGS (global):
  --base URL          Override API base
  --key KEY           Workspace API key
  --user-token TOK    User JWT / agent token
  --workspace ID      Workspace UUID

Examples:
  context-os auth login --email you@example.com --password '…'
  export CONTEXT_OS_USER_TOKEN=$(context-os auth mint --ttl 120 | jq -r .token)
  context-os workspace create --name Research
  context-os ingest --workspace $WS ./notes.pdf
  context-os ask --workspace $WS \"Summarize the docs\"

stdio MCP: context-os-mcp (same env; prefer CONTEXT_OS_USER_TOKEN for account tools)
Docs: docs/desktop/LOCAL-CLIENT-MCP-CLI-AGENT-ACCESS.md
"
    );
}

#[derive(Debug, Default)]
struct GlobalOpts {
    base: Option<String>,
    key: Option<String>,
    user_token: Option<String>,
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
    AuthLogin {
        email: String,
        password: String,
    },
    AuthMint {
        ttl_minutes: u32,
    },
    AuthWhoami,
      WorkspaceCreate {
        name: String,
        description: String,
    },
    WorkspaceList,
    ShareEnable {
        role: String,
        expires_in_secs: Option<i64>,
    },
    ShareStatus,
    ShareConfigure {
        access_level: Option<String>,
        allow_download: Option<bool>,
        anon_question_limit: Option<i32>,
        member_question_limit: Option<Option<i32>>,
        member_question_limit_set: bool,
    },
    ShareRevoke {
        token: String,
    },
    ShareQuota,
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
                opts.base = Some(args.get(i).context("--base requires a URL")?.clone());
            }
            "--key" => {
                i += 1;
                opts.key = Some(args.get(i).context("--key requires a value")?.clone());
            }
            "--user-token" | "--token" => {
                i += 1;
                opts.user_token = Some(
                    args.get(i)
                        .context("--user-token requires a value")?
                        .clone(),
                );
            }
            "--workspace" | "-w" => {
                i += 1;
                opts.workspace = Some(
                    args.get(i)
                        .context("--workspace requires a UUID")?
                        .clone(),
                );
            }
            other if other.starts_with('-') => rest.push(other.to_string()),
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
        "share" => parse_share(&mut opts, sub),
        "sources" | "list-sources" => Ok((opts, Command::Sources)),
        "auth" => parse_auth(&mut opts, sub),
        "workspace" | "ws" => parse_workspace(&mut opts, sub),
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
                    "--user-token" | "--token" => {
                        j += 1;
                        opts.user_token = Some(
                            sub.get(j)
                                .context("ask --user-token requires value")?
                                .clone(),
                        );
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
                    "--user-token" | "--token" => {
                        j += 1;
                        opts.user_token = Some(
                            sub.get(j)
                                .context("ingest --user-token requires value")?
                                .clone(),
                        );
                    }
                    part if part.starts_with('-') => bail!("unknown ingest flag: {part}"),
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

fn parse_auth(opts: &mut GlobalOpts, sub: &[String]) -> Result<(GlobalOpts, Command)> {
    if sub.is_empty() {
        bail!("auth requires a subcommand: login | mint | whoami");
    }
    match sub[0].as_str() {
        "login" => {
            let mut email = None;
            let mut password = None;
            let mut j = 1;
            while j < sub.len() {
                match sub[j].as_str() {
                    "--email" | "-e" => {
                        j += 1;
                        email = Some(sub.get(j).context("--email requires value")?.clone());
                    }
                    "--password" | "-p" => {
                        j += 1;
                        password = Some(sub.get(j).context("--password requires value")?.clone());
                    }
                    "--base" => {
                        j += 1;
                        opts.base = Some(sub.get(j).context("--base requires URL")?.clone());
                    }
                    other => bail!("unknown auth login flag: {other}"),
                }
                j += 1;
            }
            Ok((
                opts.clone(),
                Command::AuthLogin {
                    email: email.context("auth login requires --email")?,
                    password: password.context("auth login requires --password")?,
                },
            ))
        }
        "mint" | "agent-token" => {
            let mut ttl_minutes = 120u32;
            let mut j = 1;
            while j < sub.len() {
                match sub[j].as_str() {
                    "--ttl" => {
                        j += 1;
                        ttl_minutes = sub
                            .get(j)
                            .context("--ttl requires minutes")?
                            .parse()
                            .context("--ttl must be an integer")?;
                    }
                    "--user-token" | "--token" => {
                        j += 1;
                        opts.user_token = Some(
                            sub.get(j)
                                .context("--user-token requires value")?
                                .clone(),
                        );
                    }
                    "--base" => {
                        j += 1;
                        opts.base = Some(sub.get(j).context("--base requires URL")?.clone());
                    }
                    other => bail!("unknown auth mint flag: {other}"),
                }
                j += 1;
            }
            Ok((opts.clone(), Command::AuthMint { ttl_minutes }))
        }
        "whoami" | "me" | "status" => Ok((opts.clone(), Command::AuthWhoami)),
        other => bail!("unknown auth subcommand `{other}` (login|mint|whoami)"),
    }
}

fn parse_share(opts: &mut GlobalOpts, sub: &[String]) -> Result<(GlobalOpts, Command)> {
    if sub.is_empty() {
        bail!("share requires a subcommand: enable | status | configure | revoke | quota");
    }
    match sub[0].as_str() {
        "quota" => Ok((opts.clone(), Command::ShareQuota)),
        "status" | "settings" | "get" => {
            let mut j = 1;
            while j < sub.len() {
                match sub[j].as_str() {
                    "--workspace" | "-w" => {
                        j += 1;
                        opts.workspace = Some(
                            sub.get(j)
                                .context("share status --workspace requires UUID")?
                                .clone(),
                        );
                    }
                    "--user-token" => {
                        j += 1;
                        opts.user_token = Some(
                            sub.get(j)
                                .context("--user-token requires value")?
                                .clone(),
                        );
                    }
                    "--base" => {
                        j += 1;
                        opts.base = Some(sub.get(j).context("--base requires URL")?.clone());
                    }
                    other => bail!("unknown share status flag: {other}"),
                }
                j += 1;
            }
            Ok((opts.clone(), Command::ShareStatus))
        }
        "enable" | "create" | "link" => {
            let mut role = "viewer".to_string();
            let mut expires_in_secs = None;
            let mut j = 1;
            while j < sub.len() {
                match sub[j].as_str() {
                    "--role" => {
                        j += 1;
                        role = sub.get(j).context("--role requires value")?.clone();
                    }
                    "--expires-in" | "--expires-in-secs" => {
                        j += 1;
                        expires_in_secs = Some(
                            sub.get(j)
                                .context("--expires-in requires seconds")?
                                .parse()
                                .context("--expires-in must be integer")?,
                        );
                    }
                    "--workspace" | "-w" => {
                        j += 1;
                        opts.workspace = Some(
                            sub.get(j)
                                .context("share enable --workspace requires UUID")?
                                .clone(),
                        );
                    }
                    "--user-token" => {
                        j += 1;
                        opts.user_token = Some(
                            sub.get(j)
                                .context("--user-token requires value")?
                                .clone(),
                        );
                    }
                    "--base" => {
                        j += 1;
                        opts.base = Some(sub.get(j).context("--base requires URL")?.clone());
                    }
                    other => bail!("unknown share enable flag: {other}"),
                }
                j += 1;
            }
            Ok((
                opts.clone(),
                Command::ShareEnable {
                    role,
                    expires_in_secs,
                },
            ))
        }
        "configure" | "update" | "set" => {
            let mut access_level = None;
            let mut allow_download = None;
            let mut anon_question_limit = None;
            let mut member_question_limit = None;
            let mut member_question_limit_set = false;
            let mut j = 1;
            while j < sub.len() {
                match sub[j].as_str() {
                    "--access-level" | "--level" => {
                        j += 1;
                        access_level =
                            Some(sub.get(j).context("--access-level requires value")?.clone());
                    }
                    "--allow-download" => allow_download = Some(true),
                    "--no-download" => allow_download = Some(false),
                    "--anon-limit" => {
                        j += 1;
                        anon_question_limit = Some(
                            sub.get(j)
                                .context("--anon-limit requires integer")?
                                .parse()
                                .context("--anon-limit must be integer")?,
                        );
                    }
                    "--member-limit" => {
                        j += 1;
                        let raw = sub.get(j).context("--member-limit requires value")?;
                        member_question_limit_set = true;
                        if raw == "null" || raw == "unlimited" {
                            member_question_limit = None;
                        } else {
                            member_question_limit = Some(
                                raw.parse()
                                    .context("--member-limit must be integer|null|unlimited")?,
                            );
                        }
                    }
                    "--workspace" | "-w" => {
                        j += 1;
                        opts.workspace = Some(
                            sub.get(j)
                                .context("share configure --workspace requires UUID")?
                                .clone(),
                        );
                    }
                    "--user-token" => {
                        j += 1;
                        opts.user_token = Some(
                            sub.get(j)
                                .context("--user-token requires value")?
                                .clone(),
                        );
                    }
                    "--base" => {
                        j += 1;
                        opts.base = Some(sub.get(j).context("--base requires URL")?.clone());
                    }
                    other => bail!("unknown share configure flag: {other}"),
                }
                j += 1;
            }
            let member = if member_question_limit_set {
                Some(member_question_limit)
            } else {
                None
            };
            Ok((
                opts.clone(),
                Command::ShareConfigure {
                    access_level,
                    allow_download,
                    anon_question_limit,
                    member_question_limit: member,
                    member_question_limit_set,
                },
            ))
        }
        "revoke" => {
            let mut token = None;
            let mut j = 1;
            while j < sub.len() {
                match sub[j].as_str() {
                    "--token" | "--share-token" => {
                        j += 1;
                        token = Some(sub.get(j).context("--token requires value")?.clone());
                    }
                    "--workspace" | "-w" => {
                        j += 1;
                        opts.workspace = Some(
                            sub.get(j)
                                .context("share revoke --workspace requires UUID")?
                                .clone(),
                        );
                    }
                    "--user-token" => {
                        j += 1;
                        opts.user_token = Some(
                            sub.get(j)
                                .context("--user-token requires value")?
                                .clone(),
                        );
                    }
                    "--base" => {
                        j += 1;
                        opts.base = Some(sub.get(j).context("--base requires URL")?.clone());
                    }
                    other => bail!("unknown share revoke flag: {other}"),
                }
                j += 1;
            }
            Ok((
                opts.clone(),
                Command::ShareRevoke {
                    token: token.context("share revoke requires --token")?,
                },
            ))
        }
        other => bail!("unknown share subcommand `{other}` (enable|status|configure|revoke|quota)"),
    }
}

fn parse_workspace(opts: &mut GlobalOpts, sub: &[String]) -> Result<(GlobalOpts, Command)> {
    if sub.is_empty() {
        bail!("workspace requires a subcommand: create | list");
    }
    match sub[0].as_str() {
        "list" | "ls" => Ok((opts.clone(), Command::WorkspaceList)),
        "create" | "new" => {
            let mut name = None;
            let mut description = String::new();
            let mut j = 1;
            while j < sub.len() {
                match sub[j].as_str() {
                    "--name" | "-n" => {
                        j += 1;
                        name = Some(sub.get(j).context("--name requires value")?.clone());
                    }
                    "--description" | "-d" => {
                        j += 1;
                        description = sub
                            .get(j)
                            .context("--description requires value")?
                            .clone();
                    }
                    "--user-token" | "--token" => {
                        j += 1;
                        opts.user_token = Some(
                            sub.get(j)
                                .context("--user-token requires value")?
                                .clone(),
                        );
                    }
                    "--base" => {
                        j += 1;
                        opts.base = Some(sub.get(j).context("--base requires URL")?.clone());
                    }
                    other => bail!("unknown workspace create flag: {other}"),
                }
                j += 1;
            }
            Ok((
                opts.clone(),
                Command::WorkspaceCreate {
                    name: name.context("workspace create requires --name")?,
                    description,
                },
            ))
        }
        other => bail!("unknown workspace subcommand `{other}` (create|list)"),
    }
}

impl Clone for GlobalOpts {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
            key: self.key.clone(),
            user_token: self.user_token.clone(),
            workspace: self.workspace.clone(),
        }
    }
}

fn build_config(opts: &GlobalOpts) -> Result<ClientConfig> {
    Ok(ClientConfig::from_env()?
        .with_api_base(opts.base.clone())
        .with_api_key(opts.key.clone())
        .with_user_token(opts.user_token.clone())
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

    let answer = data
        .get("answer")
        .and_then(|v| v.as_str())
        .or_else(|| data.get("content").and_then(|v| v.as_str()))
        .or_else(|| data.pointer("/message/content").and_then(|v| v.as_str()));

    if let Some(text) = answer {
        println!("{text}");
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

async fn cmd_auth_login(cfg: &ClientConfig, email: &str, password: &str) -> Result<()> {
    let client = McpClient::new(cfg.clone())?;
    // Login is public — use raw HTTP without require_bearer.
    let url = format!("{}/api/auth/login", cfg.api_base);
    let resp = client
        .http()
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&json!({ "email": email, "password": password }))
        .send()
        .await
        .context("login request")?;
    let status = resp.status().as_u16();
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
    if !(200..300).contains(&status) || body.get("success") != Some(&json!(true)) {
        bail!("login failed HTTP {status}: {body}");
    }
    let token = body
        .pointer("/data/token")
        .and_then(|v| v.as_str())
        .context("login response missing data.token")?;
    println!(
        "{}",
        json!({
            "ok": true,
            "token": token,
            "user": body.pointer("/data/user"),
            "hint": "export CONTEXT_OS_USER_TOKEN=<token>  (or run auth mint for a shorter agent token)",
        })
    );
    Ok(())
}

async fn cmd_auth_mint(client: &McpClient, ttl_minutes: u32) -> Result<()> {
    let (status, body) = client
        .rest_json(
            "POST",
            "/api/auth/agent-token",
            Some(json!({ "ttl_minutes": ttl_minutes })),
            true,
        )
        .await?;
    if !(200..300).contains(&status) || body.get("success") != Some(&json!(true)) {
        let err = body
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("agent_token_failed");
        bail!("auth mint failed HTTP {status} ({err}): {body}");
    }
    let data = body.get("data").cloned().unwrap_or(json!({}));
    println!(
        "{}",
        json!({
            "ok": true,
            "token": data.get("token"),
            "expires_at": data.get("expires_at"),
            "ttl_minutes": data.get("ttl_minutes"),
            "token_kind": data.get("token_kind"),
            "hint": "export CONTEXT_OS_USER_TOKEN=<token>",
        })
    );
    Ok(())
}

async fn cmd_auth_whoami(client: &McpClient) -> Result<()> {
    let (status, body) = client
        .rest_json("GET", "/api/auth/me", None, true)
        .await?;
    if !(200..300).contains(&status) {
        bail!("whoami failed HTTP {status}: {body}");
    }
    println!("{}", serde_json::to_string_pretty(&body)?);
    Ok(())
}

async fn cmd_workspace_create(client: &McpClient, name: &str, description: &str) -> Result<()> {
    // Prefer MCP account tool so agent guide is consistent.
    let result = client
        .tools_call(
            "account.create_workspace",
            json!({ "name": name, "description": description }),
        )
        .await?;
    let data = tool_data(&result);
    println!("{}", serde_json::to_string_pretty(data)?);
    Ok(())
}

async fn cmd_workspace_list(client: &McpClient) -> Result<()> {
    let result = client
        .tools_call("account.list_workspaces", json!({}))
        .await?;
    let data = tool_data(&result);
    println!("{}", serde_json::to_string_pretty(data)?);
    Ok(())
}

async fn cmd_share_enable(
    client: &McpClient,
    role: &str,
    expires_in_secs: Option<i64>,
) -> Result<()> {
    let workspace_id = client.config().require_workspace_id()?.to_string();
    client.config().require_user_token()?;
    let mut args = json!({
        "workspace_id": workspace_id,
        "role": role,
    });
    if let Some(secs) = expires_in_secs {
        args.as_object_mut()
            .unwrap()
            .insert("expires_in_secs".into(), json!(secs));
    }
    let result = client
        .tools_call("workspace.share_create_link", args)
        .await?;
    println!("{}", serde_json::to_string_pretty(tool_data(&result))?);
    Ok(())
}

async fn cmd_share_status(client: &McpClient) -> Result<()> {
    let workspace_id = client.config().require_workspace_id()?.to_string();
    client.config().require_user_token()?;
    let result = client
        .tools_call(
            "workspace.share_get_settings",
            json!({ "workspace_id": workspace_id }),
        )
        .await?;
    println!("{}", serde_json::to_string_pretty(tool_data(&result))?);
    Ok(())
}

async fn cmd_share_configure(
    client: &McpClient,
    access_level: Option<String>,
    allow_download: Option<bool>,
    anon_question_limit: Option<i32>,
    member_question_limit: Option<Option<i32>>,
) -> Result<()> {
    let workspace_id = client.config().require_workspace_id()?.to_string();
    client.config().require_user_token()?;
    let mut args = json!({ "workspace_id": workspace_id });
    let obj = args.as_object_mut().unwrap();
    if let Some(level) = access_level {
        obj.insert("access_level".into(), json!(level));
    }
    if let Some(v) = allow_download {
        obj.insert("allow_download".into(), json!(v));
    }
    if let Some(v) = anon_question_limit {
        obj.insert("anon_question_limit".into(), json!(v));
    }
    if let Some(inner) = member_question_limit {
        match inner {
            Some(n) => obj.insert("member_question_limit".into(), json!(n)),
            None => obj.insert("member_question_limit".into(), Value::Null),
        };
    }
    let result = client
        .tools_call("workspace.share_update_settings", args)
        .await?;
    println!("{}", serde_json::to_string_pretty(tool_data(&result))?);
    Ok(())
}

async fn cmd_share_revoke(client: &McpClient, token: &str) -> Result<()> {
    let workspace_id = client.config().require_workspace_id()?.to_string();
    client.config().require_user_token()?;
    let result = client
        .tools_call(
            "workspace.share_revoke_link",
            json!({ "workspace_id": workspace_id, "token": token }),
        )
        .await?;
    println!("{}", serde_json::to_string_pretty(tool_data(&result))?);
    Ok(())
}

async fn cmd_share_quota(client: &McpClient) -> Result<()> {
    client.config().require_user_token()?;
    let result = client.tools_call("account.share_quota", json!({})).await?;
    println!("{}", serde_json::to_string_pretty(tool_data(&result))?);
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

    let cfg = match build_config(&opts) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("context-os: {e}");
            return ExitCode::from(2);
        }
    };

    let run = async {
        match command {
            Command::Status => {
                discover::run_check(&cfg)
                    .await
                    .map_err(|code| anyhow::anyhow!("status exit {code}"))?;
                Ok(())
            }
            Command::AuthLogin { email, password } => {
                cmd_auth_login(&cfg, &email, &password).await
            }
            Command::AuthMint { ttl_minutes } => {
                let client = McpClient::new(cfg)?;
                cmd_auth_mint(&client, ttl_minutes).await
            }
            Command::AuthWhoami => {
                let client = McpClient::new(cfg)?;
                cmd_auth_whoami(&client).await
            }
            Command::WorkspaceCreate { name, description } => {
                let client = McpClient::new(cfg)?;
                cmd_workspace_create(&client, &name, &description).await
            }
            Command::WorkspaceList => {
                let client = McpClient::new(cfg)?;
                cmd_workspace_list(&client).await
            }
            Command::Ingest {
                path,
                wait,
                timeout_secs,
            } => {
                let client = McpClient::new(cfg)?;
                cmd_ingest(&client, &path, wait, timeout_secs).await
            }
            Command::Ask { query } => {
                let client = McpClient::new(cfg)?;
                cmd_ask(&client, &query).await
            }
            Command::Sources => {
                let client = McpClient::new(cfg)?;
                cmd_sources(&client).await
            }
            Command::ShareEnable {
                role,
                expires_in_secs,
            } => {
                let client = McpClient::new(cfg)?;
                cmd_share_enable(&client, &role, expires_in_secs).await
            }
            Command::ShareStatus => {
                let client = McpClient::new(cfg)?;
                cmd_share_status(&client).await
            }
            Command::ShareConfigure {
                access_level,
                allow_download,
                anon_question_limit,
                member_question_limit,
                member_question_limit_set: _,
            } => {
                let client = McpClient::new(cfg)?;
                cmd_share_configure(
                    &client,
                    access_level,
                    allow_download,
                    anon_question_limit,
                    member_question_limit,
                )
                .await
            }
            Command::ShareRevoke { token } => {
                let client = McpClient::new(cfg)?;
                cmd_share_revoke(&client, &token).await
            }
            Command::ShareQuota => {
                let client = McpClient::new(cfg)?;
                cmd_share_quota(&client).await
            }
            Command::Help => unreachable!(),
        }
    };

    match run.await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Map discover exit codes
            let msg = e.to_string();
            if let Some(rest) = msg.strip_prefix("status exit ") {
                if let Ok(code) = rest.parse::<u8>() {
                    return ExitCode::from(code);
                }
            }
            eprintln!("context-os: {e}");
            ExitCode::FAILURE
        }
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
    fn parse_share_enable() {
        let (opts, cmd) = parse_args(&[
            "share".into(),
            "enable".into(),
            "--workspace".into(),
            "ws".into(),
            "--role".into(),
            "viewer".into(),
        ])
        .unwrap();
        assert_eq!(opts.workspace.as_deref(), Some("ws"));
        match cmd {
            Command::ShareEnable { role, .. } => assert_eq!(role, "viewer"),
            _ => panic!("expected share enable"),
        }
    }

    #[test]
    fn parse_share_quota() {
        let (_, cmd) = parse_args(&["share".into(), "quota".into()]).unwrap();
        assert!(matches!(cmd, Command::ShareQuota));
    }

    #[test]
    fn parse_auth_mint() {
        let (_, cmd) = parse_args(&["auth".into(), "mint".into(), "--ttl".into(), "30".into()])
            .unwrap();
        match cmd {
            Command::AuthMint { ttl_minutes } => assert_eq!(ttl_minutes, 30),
            _ => panic!("expected mint"),
        }
    }

    #[test]
    fn parse_workspace_create() {
        let (_, cmd) = parse_args(&[
            "workspace".into(),
            "create".into(),
            "--name".into(),
            "Lab".into(),
        ])
        .unwrap();
        match cmd {
            Command::WorkspaceCreate { name, .. } => assert_eq!(name, "Lab"),
            _ => panic!("expected create"),
        }
    }
}
