//! CLI argument definitions for the e2e-analyzer.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "e2e-analyzer")]
#[command(about = "Analyze E2E test artifacts from crates/app/tests/e2e_output/")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Compare two runs and list differences.
    Diff {
        /// Path to the baseline run directory.
        #[arg(long)]
        baseline: PathBuf,

        /// Path to the current run directory.
        #[arg(long)]
        current: PathBuf,

        /// Output path for the diff report (default: stdout).
        #[arg(long)]
        output: Option<PathBuf>,

        /// Minimum severity to include.
        #[arg(long, default_value = "minor")]
        min_severity: String,
    },

    /// Diagnose failures in a single run.
    Diagnose {
        /// Path to the run directory.
        #[arg(long)]
        run: PathBuf,

        /// Output path for the diagnosis report (default: stdout).
        #[arg(long)]
        output: Option<PathBuf>,

        /// Focus on a specific test name.
        #[arg(long)]
        test: Option<String>,
    },

    /// Identify coverage gaps across tests.
    Coverage {
        /// Path to the run directory or directories.
        #[arg(long)]
        runs: Vec<PathBuf>,

        /// Output path for the coverage report (default: stdout).
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Analyze trends across multiple historical runs.
    Trends {
        /// Path to the directory containing historical run directories.
        #[arg(long)]
        history: PathBuf,

        /// Output path for the trends report (default: stdout).
        #[arg(long)]
        output: Option<PathBuf>,

        /// Number of recent runs to include.
        #[arg(long, default_value = "10")]
        limit: usize,
    },

    /// Generate a comprehensive JSON or Markdown report.
    Report {
        /// Path to the run directory.
        #[arg(long)]
        run: PathBuf,

        /// Output path for the report.
        #[arg(long)]
        output: PathBuf,

        /// Report format.
        #[arg(long, value_enum, default_value = "json")]
        format: ReportFormat,
    },

    /// Summarize llm_real observability artifacts.
    LlmReal {
        #[command(subcommand)]
        command: LlmRealCommands,
    },

    /// Set or update a baseline run.
    Baseline {
        /// Path to the run directory to use as baseline.
        #[arg(long)]
        run: PathBuf,

        /// Name for the baseline (default: "default").
        #[arg(long, default_value = "default")]
        name: String,

        /// Path to the baseline storage directory.
        #[arg(long)]
        store: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
pub enum LlmRealCommands {
    /// List llm_real runs under e2e_output.
    List {
        /// Base e2e_output directory (default: crates/app/tests/e2e_output).
        #[arg(long, default_value = "crates/app/tests/e2e_output")]
        output: PathBuf,

        /// Max runs to print (newest last).
        #[arg(long, default_value = "20")]
        limit: usize,
    },

    /// Print reasoning/prompt capture stats for one llm_real run.
    Summary {
        /// Path to run directory or run id under e2e_output.
        #[arg(long)]
        run: PathBuf,

        /// Optional output path (default: stdout).
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Aggregate llm_real metadata across sparse runs (one test per run dir).
    Trends {
        /// Base e2e_output directory (default: crates/app/tests/e2e_output).
        #[arg(long, default_value = "crates/app/tests/e2e_output")]
        output: PathBuf,

        /// Max recent llm_real runs to scan.
        #[arg(long, default_value = "20")]
        limit: usize,

        /// Optional output path (default: stdout).
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ReportFormat {
    Json,
    Markdown,
}
