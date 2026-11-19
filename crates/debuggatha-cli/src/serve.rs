//! `debuggatha serve`: the engine as a process that clients talk to over Protocol v1, one JSON
//! object per line on stdin and stdout (`docs/PROTOCOL.md`). stdout carries protocol messages only.

use std::io::{BufRead, BufReader, Read, Write};
use std::sync::{Arc, Mutex};

use clap::{Args as ClapArgs, ValueEnum};
use debuggatha_engine::analyzers::{ADAPTERS, path_dirs};
use debuggatha_engine::daemon::{Config, Daemon, Emitter, MAX_LINE_BYTES, Request};
use debuggatha_engine::semantic_layer::{LocalRuntime, connect_local};
use serde_json::Value;

use crate::Context;

#[derive(Clone, Copy, ValueEnum)]
enum Runtime {
    /// No model, which is also what leaving the option out means.
    Off,
    Ollama,
    Lmstudio,
    /// The model of the client that talks to the daemon, if it offers one (`sampling` too).
    #[value(alias = "sampling")]
    Host,
}

#[derive(ClapArgs)]
pub(crate) struct Args {
    /// Analyzers clients may run although they load project code or use the network. Only the
    /// person starting the daemon can allow them; a request never can. `auto` (the default) adds
    /// none: the tools that only read source always run unless a request turns them off.
    #[arg(long, env = "DEBUGGATHA_ANALYZERS", value_delimiter = ',', num_args = 1.., value_parser = clap::builder::PossibleValuesParser::new(ADAPTERS.iter().map(|a| a.id).chain(["auto"])))]
    analyzer: Vec<String>,
    /// A model clients may ask to read the changed code: one on this machine, or their own.
    #[arg(long, env = "DEBUGGATHA_SEMANTIC", value_enum)]
    semantic: Option<Runtime>,
    #[arg(long, env = "DEBUGGATHA_SEMANTIC_MODEL", requires = "semantic")]
    semantic_model: Option<String>,
    #[arg(long, env = "DEBUGGATHA_SEMANTIC_URL", requires = "semantic")]
    semantic_url: Option<String>,
}

fn send(out: &Mutex<std::io::Stdout>, message: &Value) {
    let mut out = out
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // A client that closed its end is gone; there is no one to tell.
    drop(writeln!(out, "{message}"));
    drop(out.flush());
}

pub(crate) enum Line {
    End,
    TooLong,
    Read,
}

/// One line, at most `MAX_LINE_BYTES`.
pub(crate) fn read_line(input: &mut impl BufRead, buffer: &mut Vec<u8>) -> Line {
    buffer.clear();
    let limit = u64::try_from(MAX_LINE_BYTES).unwrap_or(u64::MAX) + 1;
    match input.by_ref().take(limit).read_until(b'\n', buffer) {
        Ok(0) | Err(_) => Line::End,
        Ok(_) if buffer.last() != Some(&b'\n') && buffer.len() > MAX_LINE_BYTES => Line::TooLong,
        Ok(_) => Line::Read,
    }
}

/// What the person who starts the daemon allows; shared by `serve` and `mcp`.
pub(crate) fn config(context: &Context, args: &Args) -> Result<Config, String> {
    config_at(context.root()?, args)
}

/// The same for a repository other than the one the command was started in.
pub(crate) fn config_at(root: std::path::PathBuf, args: &Args) -> Result<Config, String> {
    let client_model = matches!(args.semantic, Some(Runtime::Host));
    if client_model && (args.semantic_url.is_some() || args.semantic_model.is_some()) {
        return Err("The client's own model is chosen by the client, not by --semantic-model or --semantic-url.".to_owned());
    }
    let local = match args.semantic {
        Some(Runtime::Ollama) => Some(LocalRuntime::Ollama),
        Some(Runtime::Lmstudio) => Some(LocalRuntime::LmStudio),
        Some(Runtime::Host | Runtime::Off) | None => None,
    };
    let semantic = local
        .map(|runtime| {
            connect_local(
                runtime,
                args.semantic_url.as_deref(),
                args.semantic_model.as_deref(),
                None,
            )
        })
        .transpose()?;
    Ok(Config {
        root,
        enabled_analyzers: args
            .analyzer
            .iter()
            .filter(|id| *id != "auto")
            .cloned()
            .collect(),
        analyzer_search: path_dirs(),
        semantic,
        client_model,
    })
}

pub(crate) fn run(context: &Context, args: &Args) -> Result<bool, String> {
    let daemon = Daemon::new(config(context, args)?);
    let out = Arc::new(Mutex::new(std::io::stdout()));
    let emit: Emitter = {
        let out = Arc::clone(&out);
        Arc::new(move |event| send(&out, &event))
    };
    let mut input = BufReader::new(std::io::stdin().lock());
    let mut buffer = Vec::new();

    std::thread::scope(|scope| {
        loop {
            match read_line(&mut input, &mut buffer) {
                Line::End => break,
                Line::TooLong => {
                    send(&out, &daemon.oversized_line());
                    break;
                }
                Line::Read => {}
            }
            let text = String::from_utf8_lossy(&buffer);
            if daemon.event(text.trim_end()) {
                continue;
            }
            let request: Request = match daemon.parse(text.trim_end()) {
                Ok(request) => request,
                Err(Some(response)) => {
                    send(&out, &response);
                    continue;
                }
                Err(None) => continue,
            };
            // The handshake and the end of the session are ordered; the rest run side by side.
            if request.method == "hello" || Daemon::is_shutdown(&request) {
                let response = daemon.handle(&request, &emit);
                if let Some(response) = &response {
                    send(&out, response);
                }
                if Daemon::is_shutdown(&request)
                    || response
                        .as_ref()
                        .is_some_and(|r| daemon.handshake_failed(&request, r))
                {
                    break;
                }
            } else {
                let (daemon, out, emit) = (&daemon, &out, &emit);
                scope.spawn(move || {
                    if let Some(response) = daemon.handle(&request, emit) {
                        send(out, &response);
                    }
                });
            }
        }
        // End of input or shutdown: whatever still runs is stopped, not waited for.
        daemon.cancel_all();
    });
    Ok(false)
}
