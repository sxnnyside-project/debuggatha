//! The `debuggatha` binary.

mod analyzers;
mod baseline;
mod completion;
mod config;
mod doctor;
mod findings;
mod formats;
mod init;
mod mcp;
mod mcp_http;
mod mcp_input;
mod mcp_server;
mod memory;
mod packs;
mod policies;
mod report;
mod repository;
mod review;
mod serve;
mod trends;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Exit codes of every command: `0` clean, `2` findings at or above the threshold, `1` error.
const FINDINGS: u8 = 2;
const ERROR: u8 = 1;

#[derive(Parser)]
#[command(name = "debuggatha", version = debuggatha_engine::VERSION, about = "A senior code reviewer that cites a real source for every finding.")]
pub(crate) struct Cli {
    /// The repository the command works on.
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    /// How to print: `review` knows all of them, the other commands only text and json.
    #[arg(long, global = true, value_enum)]
    format: Option<report::ReportFormat>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Review the working tree, the given files, or the whole repository.
    Review(Box<review::Args>),
    /// List the external analyzers: what is installed here and what a review would run.
    Analyzers(analyzers::Args),
    /// Accept the findings a repository has today, so reviews report only what is new.
    #[command(subcommand)]
    Baseline(baseline::Command),
    /// Query and manage the Findings Ledger.
    #[command(subcommand)]
    Findings(findings::Command),
    /// Inspect and manage Repository Memory.
    #[command(subcommand)]
    Memory(memory::Command),
    /// List the Review Packs the engine ships.
    Packs,
    /// Show the Review Policy a review of this repository would assemble now.
    Policies,
    /// Inspect what Debuggatha understands of the repository.
    Repository,
    /// Verify the repository, its stored files and the environment.
    Doctor,
    /// How the findings are moving: open now, introduced and resolved per week, time to fix.
    Trends(trends::Args),
    /// Set a repository up: a config file, a CI workflow and optionally a pre-commit hook.
    Init(init::Args),
    /// Run the engine as an MCP server on stdio, for hosts such as Claude Code, Cursor or Copilot.
    Mcp(mcp::Options),
    /// Run the engine as a process clients talk to over Protocol v1 (stdin and stdout).
    Serve(serve::Args),
    /// Print a shell completion script.
    Completion {
        #[arg(value_enum)]
        shell: completion::Target,
    },
}

/// What every command shares: the repository and how to print.
pub(crate) struct Context {
    root: PathBuf,
    /// Did the person name a repository, or is it the current directory by default?
    root_given: bool,
    report: Option<report::ReportFormat>,
}

/// What an error may say to the person: no paths, no internals (`client_error`).
pub(crate) fn client_message<E: std::error::Error + 'static>(error: &E) -> String {
    debuggatha_engine::client_error::classify_error(error).client_message
}

impl Context {
    /// What the person asked for, if they said.
    pub(crate) fn report(&self) -> Option<report::ReportFormat> {
        self.report
    }

    /// Text or json: what commands other than `review` can print.
    pub(crate) fn format(&self) -> report::Format {
        match self.report {
            Some(report::ReportFormat::Json) => report::Format::Json,
            _ => report::Format::Text,
        }
    }

    pub(crate) fn root_given(&self) -> bool {
        self.root_given
    }

    /// The repository root, which must exist.
    pub(crate) fn root(&self) -> Result<PathBuf, String> {
        self.root
            .canonicalize()
            .map_err(|_| "The repository path does not exist or cannot be read.".to_owned())
    }
}

fn main() -> ExitCode {
    // clap exits with 2 on a usage error, which here means "findings".
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let code = if error.use_stderr() { ERROR } else { 0 };
            drop(error.print());
            return ExitCode::from(code);
        }
    };
    let context = Context {
        root_given: cli.root.is_some(),
        root: cli.root.unwrap_or_else(|| PathBuf::from(".")),
        report: cli.format,
    };
    let outcome = match cli.command {
        Command::Review(args) => review::run(&context, &args),
        Command::Analyzers(args) => analyzers::run(&context, &args),
        Command::Baseline(command) => baseline::run(&context, &command),
        Command::Findings(command) => findings::run(&context, &command),
        Command::Memory(command) => memory::run(&context, &command),
        Command::Packs => {
            packs::run(&context);
            Ok(false)
        }
        Command::Policies => policies::run(&context),
        Command::Repository => repository::run(&context),
        Command::Doctor => doctor::run(&context),
        Command::Init(args) => init::run(&context, &args),
        Command::Serve(args) => serve::run(&context, &args),
        Command::Mcp(args) => mcp::run(&context, &args),
        Command::Trends(args) => trends::run(&context, &args),
        Command::Completion { shell } => {
            completion::run(shell);
            Ok(false)
        }
    };
    match outcome {
        Ok(true) => ExitCode::from(FINDINGS),
        Ok(false) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("debuggatha: {message}");
            ExitCode::from(ERROR)
        }
    }
}
