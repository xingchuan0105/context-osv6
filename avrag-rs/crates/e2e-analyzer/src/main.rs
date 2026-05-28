//! e2e-analyzer — CLI tool for analyzing E2E test artifacts.

mod baseline;
mod cli;
mod diff;
mod fingerprint;
mod loader;
mod models;
mod report;

use clap::Parser;
use cli::{Cli, Commands};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Diff {
            baseline,
            current,
            output,
            min_severity,
        } => {
            println!("diff: baseline={baseline:?} current={current:?} output={output:?} min_severity={min_severity}");
            // TODO: implement diff logic
        }
        Commands::Diagnose { run, output, test } => {
            println!("diagnose: run={run:?} output={output:?} test={test:?}");
            // TODO: implement diagnose logic
        }
        Commands::Coverage { runs, output } => {
            println!("coverage: runs={runs:?} output={output:?}");
            // TODO: implement coverage logic
        }
        Commands::Trends {
            history,
            output,
            limit,
        } => {
            println!("trends: history={history:?} output={output:?} limit={limit}");
            // TODO: implement trends logic
        }
        Commands::Report {
            run,
            output,
            format,
        } => {
            println!("report: run={run:?} output={output:?} format={format:?}");
            // TODO: implement report logic
        }
        Commands::Baseline { run, name, store } => {
            println!("baseline: run={run:?} name={name} store={store:?}");
            // TODO: implement baseline logic
        }
    }

    Ok(())
}
