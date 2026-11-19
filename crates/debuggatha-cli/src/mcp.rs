//! `debuggatha mcp`: the engine as an MCP server on stdio (newline-delimited JSON-RPC 2.0). It is
//! an adapter: each tool is a Protocol v1 method of the same daemon `serve` runs, and nothing is
//! decided here. A client that can sample is the model of the semantic pass; the daemon asks
//! for it with `model/complete` and this module turns that into `sampling/createMessage`.

use std::io::BufReader;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use clap::Args as ClapArgs;
use debuggatha_engine::daemon::Daemon;
use serde_json::{Value, json};

use crate::Context;
use crate::mcp_server::{Make, Out, Repositories, Server, send};
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

/// The protocol versions this server speaks, newest first. A client asking for one of them gets it.
pub(crate) const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

pub(crate) const INSTRUCTIONS: &str = "Debuggatha reviews code against the Review Packs that match the repository's stack, and every finding cites the rule and evidence behind it. After you write or edit code, call review_changes, fix what changes.introduced lists (each recommendation says how, sometimes with an exact edit), and call it again until nothing new is introduced; changes.fixed confirms what your fixes resolved. Use explain_finding for the reasoning behind a finding. Accept a finding with suppress_finding only when the code is right and the rule is wrong, always with a real reason, and tell the user. Review tools record findings in the repository's ledger unless persist is false. When the client has a model, semantic: true also has it read the changed code; what it raises is a labeled suspicion to check, never a verdict.";

pub(crate) fn rpc_error(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

pub(crate) fn rpc_result(id: &Value, result: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn schema(properties: &[(&str, Value)], required: &[&str]) -> Value {
    let properties: serde_json::Map<String, Value> = properties
        .iter()
        .map(|(name, schema)| ((*name).to_owned(), schema.clone()))
        .collect();
    json!({ "type": "object", "properties": properties, "required": required })
}

fn text(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

fn flag(description: &str) -> Value {
    json!({ "type": "boolean", "description": description })
}

fn list(description: &str, item: &Value) -> Value {
    json!({ "type": "array", "items": item, "description": description })
}

fn review_properties(extra: Vec<(&'static str, Value)>) -> Vec<(&'static str, Value)> {
    let mut all = extra;
    all.extend([
        ("packIds", list("Extra Review Pack ids on top of the packs detected for the repository's stack. Listed packs win rule conflicts.", &json!({ "type": "string" }))),
        ("policyId", text("Assert the review policy id a previous review reported; fails if it changed.")),
        ("persist", flag("Record findings in .debuggatha/ledger.json. Defaults to true; false is read-only.")),
        ("includeBaselined", flag("Also report findings the repository's baseline accepted. Defaults to false: with a baseline, only new findings are reported.")),
        ("semantic", flag("Also have the client's model read the changed code for defects analyzers do not see. What it raises is a labeled suspicion with low confidence, each quoting the line it is about; it never closes or hides a finding.")),
        ("analyzers", flag("Set false to run only the built-in detectors. Which external analyzers run otherwise is decided when the server starts; a call cannot enable more.")),
    ]);
    all
}

const STATUSES: [&str; 5] = ["open", "acknowledged", "resolved", "dismissed", "reopened"];
const SEVERITIES: [&str; 5] = ["informational", "low", "medium", "high", "critical"];

/// How a tool becomes a daemon request.
pub(crate) enum Call {
    /// A review: the scope the tool names, and the parameters every review takes.
    Review(fn(&Value) -> Result<Value, String>),
    /// Any other method; the arguments are renamed to the daemon's by `args`.
    Method(&'static str, fn(&Value) -> Result<Value, String>),
}

pub(crate) struct Tool {
    pub(crate) name: &'static str,
    pub(crate) title: &'static str,
    pub(crate) description: &'static str,
    pub(crate) input: fn() -> Value,
    pub(crate) read_only: bool,
    pub(crate) destructive: bool,
    pub(crate) call: Call,
}

fn require<'a>(args: &'a Value, name: &str) -> Result<&'a Value, String> {
    args.get(name)
        .filter(|v| !v.is_null())
        .ok_or_else(|| format!("The argument \"{name}\" is required."))
}

fn pick(args: &Value, names: &[&str]) -> Value {
    let mut out = serde_json::Map::new();
    for name in names {
        if let Some(value) = args.get(*name).filter(|v| !v.is_null()) {
            out.insert((*name).to_owned(), value.clone());
        }
    }
    Value::Object(out)
}

fn finding_id(args: &Value) -> Result<Value, String> {
    Ok(json!({ "id": require(args, "findingId")? }))
}

pub(crate) fn tools() -> Vec<Tool> {
    let mut all = review_tools();
    all.extend(finding_tools());
    all.extend(ledger_tools());
    all.extend(repository_tools());
    all
}

fn review_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "review_changes",
            title: "Review what changed",
            description: "Call this after you write or edit code. Reviews the files changed in the working tree since the last commit (or since `base`): edits, new files git does not track yet, and deletions. The response's `changes` says what your work introduced, what it fixed (findings that are gone because the code was corrected or the file removed), and what came back. Fix what you introduced, then call it again until `changes.introduced` is empty.",
            input: || {
                schema(
                    &review_properties(vec![(
                        "base",
                        text(
                            "A git ref or range to measure changes against, e.g. origin/main...HEAD. Defaults to the last commit.",
                        ),
                    )]),
                    &[],
                )
            },
            read_only: false,
            destructive: false,
            call: Call::Review(|args| {
                Ok(
                    json!({ "kind": "diff", "base": args.get("base").cloned().unwrap_or(Value::Null) }),
                )
            }),
        },
        Tool {
            name: "review_diff",
            title: "Review a git diff",
            description: "Reviews the added lines of a unified git diff against the Review Packs that match the repository's stack. Every finding cites the rule that produced it.",
            input: || {
                schema(
                    &review_properties(vec![
                        ("diff", text("The raw unified git diff text.")),
                        (
                            "base",
                            text("The base commit or branch the diff was taken against."),
                        ),
                    ]),
                    &["diff"],
                )
            },
            read_only: false,
            destructive: false,
            call: Call::Review(|args| {
                Ok(json!({ "kind": "diff", "diff": require(args, "diff")? }))
            }),
        },
        Tool {
            name: "review_files",
            title: "Review specific files",
            description: "Reviews whole files against the Review Packs that match the repository's stack.",
            input: || {
                schema(
                    &review_properties(vec![(
                        "paths",
                        list(
                            "Paths of the files to review, inside the repository.",
                            &json!({ "type": "string" }),
                        ),
                    )]),
                    &["paths"],
                )
            },
            read_only: false,
            destructive: false,
            call: Call::Review(|args| {
                Ok(json!({ "kind": "files", "paths": require(args, "paths")? }))
            }),
        },
        Tool {
            name: "review_workspace",
            title: "Review the whole repository",
            description: "Reviews every source file in the repository, plus its architecture: dependency cycles, layer violations, and dead code.",
            input: || schema(&review_properties(vec![]), &[]),
            read_only: false,
            destructive: false,
            call: Call::Review(|_| Ok(json!({ "kind": "workspace" }))),
        },
    ]
}

fn finding_tools() -> Vec<Tool> {
    let id_only = || {
        schema(
            &[("findingId", text("The finding's ledgerId."))],
            &["findingId"],
        )
    };
    vec![
        Tool {
            name: "explain_finding",
            title: "Explain a finding",
            description: "Everything needed to act on one finding: where it is, why the pattern is a problem, how to fix it (an exact edit when the fix is mechanical, otherwise a before/after example), and what to do if the code is right and the rule is wrong.",
            input: id_only,
            read_only: true,
            destructive: false,
            call: Call::Method("findings/explain", finding_id),
        },
        Tool {
            name: "suppress_finding",
            title: "Accept a finding, with a reason",
            description: "Records that a finding is accepted: every finding of the same rule in the same file stops being reported, and the reason is kept in .debuggatha/memory.json for the team to read. Use it only when the code is right and the rule is wrong for this case, never to make a review quiet. A reason is required. Prefer fixing the code; to accept one line instead, add the inline comment explain_finding gives you.",
            input: || {
                schema(
                    &[
                        ("findingId", text("The finding's ledgerId.")),
                        (
                            "reason",
                            json!({ "type": "string", "minLength": 8, "description": "Why this is acceptable: what makes the rule wrong here. Shown to reviewers." }),
                        ),
                    ],
                    &["findingId", "reason"],
                )
            },
            read_only: false,
            destructive: true,
            call: Call::Method("findings/suppress", |args| {
                Ok(json!({ "id": require(args, "findingId")?, "reason": require(args, "reason")? }))
            }),
        },
    ]
}

fn ledger_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "list_findings",
            title: "List findings",
            description: "Lists the findings the repository's ledger tracks, filtered by lifecycle status, severity, category or file.",
            input: || {
                schema(
                    &[
                        (
                            "status",
                            list(
                                "Only entries in these lifecycle states.",
                                &json!({ "type": "string", "enum": STATUSES }),
                            ),
                        ),
                        (
                            "severity",
                            list(
                                "Only entries of these severities.",
                                &json!({ "type": "string", "enum": SEVERITIES }),
                            ),
                        ),
                        (
                            "category",
                            list(
                                "Only entries in these categories.",
                                &json!({ "type": "string" }),
                            ),
                        ),
                        (
                            "file",
                            text("Only entries in this file, as a repository-relative path."),
                        ),
                        (
                            "limit",
                            json!({ "type": "integer", "minimum": 1, "description": "At most this many entries." }),
                        ),
                        ("cursor", text("The nextCursor of the previous page.")),
                    ],
                    &[],
                )
            },
            read_only: true,
            destructive: false,
            call: Call::Method("findings/list", |args| {
                Ok(pick(
                    args,
                    &["status", "severity", "category", "file", "limit", "cursor"],
                ))
            }),
        },
        Tool {
            name: "get_finding",
            title: "Get a finding",
            description: "Returns one ledger entry, including its full lifecycle history.",
            input: || {
                schema(
                    &[("findingId", text("The ledger entry id."))],
                    &["findingId"],
                )
            },
            read_only: true,
            destructive: false,
            call: Call::Method("findings/get", finding_id),
        },
        Tool {
            name: "update_finding",
            title: "Update a finding's status",
            description: "Moves a ledger entry to a new lifecycle status and records the change in its history. Illegal transitions are rejected.",
            input: || {
                schema(
                    &[
                        ("findingId", text("The ledger entry id.")),
                        (
                            "status",
                            json!({ "type": "string", "enum": STATUSES, "description": "The status to move the finding to." }),
                        ),
                        (
                            "comment",
                            text("Why the status changed; stored in the history."),
                        ),
                    ],
                    &["findingId", "status"],
                )
            },
            read_only: false,
            destructive: false,
            call: Call::Method("findings/update", |args| {
                let mut params = pick(args, &["status", "comment"]);
                params["id"] = require(args, "findingId")?.clone();
                Ok(params)
            }),
        },
    ]
}

fn repository_tools() -> Vec<Tool> {
    let none = || schema(&[], &[]);
    vec![
        Tool {
            name: "create_baseline",
            title: "Create a baseline",
            description: "Reviews the whole repository and records every finding it has today in .debuggatha/baseline.json. From then on, review tools report only findings that are new since the baseline (pass includeBaselined to see all). Use it to adopt Debuggatha on an existing repository without drowning in old findings.",
            input: none,
            read_only: false,
            destructive: false,
            call: Call::Method("baseline/create", |_| Ok(json!({}))),
        },
        Tool {
            name: "list_analyzers",
            title: "List analyzers",
            description: "Lists the external analyzers (Ruff, Biome, gitleaks, ktlint, and others) that reviews can run: which are installed here, each one's license, and which a review would run. Debuggatha runs them as separate processes; it does not include or install them. Tools that run project code or use the network run only when the server was started with them enabled.",
            input: none,
            read_only: true,
            destructive: false,
            call: Call::Method("analyzers/list", |_| Ok(json!({}))),
        },
        Tool {
            name: "repository_context",
            title: "Repository context",
            description: "The full context Debuggatha resolves before reviewing: stack, dependencies, documentation, criteria, and understanding.",
            input: none,
            read_only: true,
            destructive: false,
            call: Call::Method("repository/context", |_| Ok(json!({}))),
        },
        Tool {
            name: "repository_summary",
            title: "Repository summary",
            description: "A compact view of the repository: detected stack and capabilities, dependencies, criteria, and ledger totals.",
            input: none,
            read_only: true,
            destructive: false,
            call: Call::Method("repository/summary", |_| Ok(json!({}))),
        },
    ]
}

/// The parameters of `review/run`: the scope the tool named and what every review takes.
pub(crate) fn review_params(scope: Value, args: &Value) -> Value {
    let mut params = pick(
        args,
        &[
            "packIds",
            "policyId",
            "persist",
            "includeBaselined",
            "semantic",
        ],
    );
    params["scope"] = scope;
    if args.get("analyzers") == Some(&json!(false)) {
        params["analyzers"] = json!({ "mode": "off" });
    }
    params
}

pub(crate) fn prompt_text(name: &str, args: &Value) -> Option<String> {
    let first = "Call `review_changes`. It reviews what changed in the working tree, including new files and deletions.";
    let explain = "Call `explain_finding` with its `ledgerId`.";
    let loop_of = |step: &str| {
        [
            format!("1. {step}"),
            "2. Fix the code for each finding in `changes.introduced` (and `changes.reopened`). Use the recommendation: apply its `edit` when it has one (check the result when `safety` is `review`), otherwise follow its `example`. Call `explain_finding` with the finding's `ledgerId` if you need the reasoning.".to_owned(),
            "3. Call `review_changes` again. `changes.fixed` should now list what you fixed and `changes.introduced` should be empty. Repeat until it is, or until a finding needs a decision from the user.".to_owned(),
            "4. Only when the code is right and the rule is wrong for this case, accept the finding with `suppress_finding` and a real reason, or with the inline comment `explain_finding` gives you. Never accept a finding just to make a review quiet, and tell the user every finding you accepted and why.".to_owned(),
        ]
        .join("\n")
    };
    match name {
        "review-flow" => Some(format!(
            "Review the code you just changed with Debuggatha, and fix what it finds:\n\n{}\n\nFinish by summarizing what was introduced, what you fixed, and what you accepted.",
            loop_of(first)
        )),
        "fix-findings" => {
            let severity = args["severity"]
                .as_str()
                .map(|s| format!(", severity {s} and above"))
                .unwrap_or_default();
            Some(format!(
                "List the open findings with `list_findings` (status \"open\"{severity}), then fix them starting with the most severe. For each one:\n\n{}",
                loop_of(explain)
            ))
        }
        "triage-findings" => Some(
            "List the open findings with the `list_findings` tool (status \"open\"), then propose resolving or dismissing each with `update_finding`.".to_owned(),
        ),
        _ => None,
    }
}

pub(crate) struct Resource {
    pub(crate) uri: &'static str,
    pub(crate) name: &'static str,
    pub(crate) title: &'static str,
    pub(crate) description: &'static str,
    pub(crate) method: &'static str,
    /// Every page of a listing, not the first.
    all: bool,
}

impl Resource {
    pub(crate) fn params(&self) -> Value {
        if self.all {
            json!({ "limit": u32::MAX })
        } else {
            json!({})
        }
    }
}

pub(crate) const RESOURCES: [Resource; 4] = [
    Resource {
        uri: "debuggatha://repository/context",
        name: "repository-context",
        title: "Repository context",
        description: "Stack, dependencies, documentation, criteria, and understanding.",
        method: "repository/context",
        all: false,
    },
    Resource {
        uri: "debuggatha://repository/summary",
        name: "repository-summary",
        title: "Repository summary",
        description: "Stack, dependencies, criteria, and ledger totals.",
        method: "repository/summary",
        all: false,
    },
    Resource {
        uri: "debuggatha://ledger",
        name: "ledger",
        title: "Findings ledger",
        description: "Every finding the repository's ledger tracks.",
        method: "findings/list",
        all: true,
    },
    Resource {
        uri: "debuggatha://ledger/history",
        name: "ledger-history",
        title: "Findings lifecycle history",
        description: "Every lifecycle event across all findings, newest first.",
        method: "findings/history",
        all: false,
    },
];

pub(crate) fn resources() -> Value {
    Value::Array(
        RESOURCES
            .iter()
            .map(|r| {
                json!({ "uri": r.uri, "name": r.name, "title": r.title,
                        "description": r.description, "mimeType": "application/json" })
            })
            .collect(),
    )
}

pub(crate) fn prompts() -> Value {
    json!([
        { "name": "review-flow", "title": "Review and fix what you changed",
          "description": "After writing or editing code: review the changes, fix what the review finds, and verify the fixes." },
        { "name": "fix-findings", "title": "Fix the open findings",
          "description": "Work through the findings the ledger still has open, most severe first.",
          "arguments": [{ "name": "severity", "description": "Only findings of this severity and above (critical, high, medium or low).", "required": false }] },
        { "name": "triage-findings", "title": "Triage open findings",
          "description": "List the open findings and decide, one by one, whether to resolve or dismiss them." },
    ])
}

/// What a daemon response says to an MCP client: the result as structured content and as text,
/// or the message of the error.
pub(crate) fn tool_result(response: &Value) -> Value {
    if response["ok"] == json!(true) {
        let result = &response["result"];
        json!({
            "content": [{ "type": "text", "text": result.to_string() }],
            "structuredContent": result,
        })
    } else {
        let message = response["error"]["message"]
            .as_str()
            .unwrap_or("The call failed.");
        json!({ "isError": true, "content": [{ "type": "text", "text": message }] })
    }
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
        return crate::mcp_http::serve(repositories, &options.host, options.port);
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
