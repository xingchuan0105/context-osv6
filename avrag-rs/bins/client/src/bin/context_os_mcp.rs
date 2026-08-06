//! context-os-mcp — stdio MCP transport → local HTTP MCP gateway.

use std::env;
use std::process::ExitCode;

use context_os::config::ClientConfig;
use context_os::{discover, proxy};

fn print_usage() {
    eprintln!(
        "\
context-os-mcp — Context-OS local MCP (stdio → HTTP)

USAGE:
  context-os-mcp              Run stdio MCP proxy (default)
  context-os-mcp --check      Probe local API health and auth readiness
  context-os-mcp --help       Show this help

ENV:
  CONTEXT_OS_API_BASE         API base URL (default http://127.0.0.1:18080)
  AVRAG_PUBLIC_BASE_URL       Fallback base URL (desktop client.env)
  CONTEXT_OS_API_KEY          Workspace API key (Bearer)
  CONTEXT_OS_WORKSPACE_API_KEY  Alias for CONTEXT_OS_API_KEY

CLI companion: context-os status|ingest|ask|sources
Docs: docs/desktop/LOCAL-CLIENT-MCP-CLI-AGENT-ACCESS.md
Wire: /docs/api-access-for-agents.md
"
    );
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        return ExitCode::SUCCESS;
    }

    let cfg = match ClientConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("context-os-mcp: {e}");
            return ExitCode::from(2);
        }
    };

    if args.iter().any(|a| a == "--check" || a == "check") {
        return match discover::run_check(&cfg).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(code) => ExitCode::from(code),
        };
    }

    if let Some(unknown) = args.first() {
        eprintln!("context-os-mcp: unknown argument `{unknown}` (try --help)");
        return ExitCode::from(2);
    }

    if let Err(e) = proxy::run_stdio_proxy(cfg).await {
        eprintln!("context-os-mcp: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
