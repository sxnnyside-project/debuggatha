//! `debuggatha mcp`: the engine as an MCP server on stdio (newline-delimited JSON-RPC 2.0). It is
//! an adapter: each tool is a Protocol v1 method of the same daemon `serve` runs, and nothing is
//! decided here. A client that can sample is the model of the semantic pass; the daemon asks
//! for it with `model/complete` and this module turns that into `sampling/createMessage`.

use std::io::BufReader;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use clap::Args as ClapArgs;
use debuggatha_engine::daemon::Daemon;
use serde_json::Value;

mod http;
mod input;
mod prompts;
mod protocol;
mod resources;
mod server;
mod tools;

use crate::Context;
use crate::mcp::protocol::rpc_error;
use crate::mcp::server::{Make, Out, Repositories, Server, send};
use crate::serve::{Line, read_line};

#[derive(ClapArgs)]
pub(crate) struct Options {
    /// The same choices as `serve`: what the person allows analyzers and a local model to do.
    #[command(flatten)]
    serve: crate::serve::Args,
    /// Serve Streamable HTTP at /mcp instead of stdio. Stateless: no model of the client, no
    /// progress, no cancellation.
    #[arg(long)]
    http: bool,
    /// The interface for --http. Only a loopback address is accepted: there is no sign-in.
    #[arg(long, default_value = "127.0.0.1", requires = "http")]
    host: String,
    #[arg(long, default_value_t = 3333, requires = "http")]
    port: u16,
    /// A repository, besides the one the server starts in, that calls may name with `rootDir`
    /// (and anything under it). The roots a client shares are allowed too, over stdio.
    #[arg(long = "allow-root")]
    allow_root: Vec<PathBuf>,
}

pub(crate) fn run(context: &Context, options: &Options) -> Result<bool, String> {
    let startup = context.root()?;
    let allowed = options
        .allow_root
        .iter()
        .map(|path| {
            path.canonicalize().map_err(|_| {
                "A path given to --allow-root does not exist or cannot be read.".to_owned()
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let make: Make<'_> = Box::new(|root| {
        let mut config = crate::serve::config_at(root.to_path_buf(), &options.serve)?;
        // Whoever connected this client to the server chose its model for the review. Over HTTP
        // it is reached only by a call that declares it can sample and answers by retrying.
        config.client_model = true;
        Ok(Daemon::new(config))
    });
    let repositories = Repositories::new(make, startup, allowed, !context.root_given());
    if options.http {
        return crate::mcp::http::serve(repositories, &options.host, options.port);
    }
    stdio(repositories);
    Ok(false)
}

fn stdio(repositories: Repositories<'_>) {
    let out: Out = Arc::new(Mutex::new(std::io::stdout()));
    let server = Server::new(repositories, Some(Arc::clone(&out)));
    let mut input = BufReader::new(std::io::stdin().lock());
    let mut buffer = Vec::new();

    std::thread::scope(|scope| {
        loop {
            match read_line(&mut input, &mut buffer) {
                Line::End => break,
                Line::TooLong => {
                    send(
                        &out,
                        &rpc_error(&Value::Null, -32600, "A message is longer than 16 MiB."),
                    );
                    break;
                }
                Line::Read => {}
            }
            let text = String::from_utf8_lossy(&buffer);
            let Ok(message) = serde_json::from_str::<Value>(text.trim_end()) else {
                if !text.trim().is_empty() {
                    send(
                        &out,
                        &rpc_error(&Value::Null, -32700, "The message is not JSON."),
                    );
                }
                continue;
            };
            if message.get("method").is_none() && message.get("id").is_some() {
                server.answer(&message);
                continue;
            }
            let (server, out) = (&server, &out);
            scope.spawn(move || {
                if let Some(response) = server.handle(&message) {
                    send(out, &response);
                }
            });
        }
        // End of input: whatever still runs is stopped, not waited for.
        server.cancel_all();
    });
}
