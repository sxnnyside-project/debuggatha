//! The thirteen tools: what each takes, returns and does to the repository, and the daemon method it calls.

use serde_json::{Value, json};

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
        ("semantic", flag("Also have the client's model read the changed code for defects analyzers do not see. Needs a client that can be asked for its own model (sampling, or input-required on MCP 2026-07-28); without one the call fails with a message saying so and nothing is recorded. It costs one or more model calls of the client and is slower. What it raises is a labeled suspicion with low confidence, each quoting the line it is about; it never closes or hides a finding.")),
        ("limit", json!({ "type": "integer", "minimum": 1, "description": "At most this many findings in findingsPage, highest severity first (default 100). findingsPage.truncated says whether more exist; list_findings pages through all of them." })),
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

fn object(properties: &Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": properties, "required": required })
}

/// What a tool's structured result holds at the top level. The shapes inside are the schemas in
/// docs/schemas; they are not repeated here.
pub(crate) fn output_schema(name: &str) -> Value {
    let any = json!({});
    let finding_ref = json!({ "type": "object", "description": "A ledger entry: ledgerId, ruleId, title, file, line, severity." });
    match name {
        "review_changes" | "review_diff" | "review_files" | "review_workspace" => object(
            &json!({
                "summary": { "type": "object", "description": "Counts of the review." },
                "changes": object(&json!({
                    "introduced": { "type": "array", "items": finding_ref, "description": "What this work introduced: fix these." },
                    "fixed": { "type": "array", "items": finding_ref },
                    "reopened": { "type": "array", "items": finding_ref },
                }), &[]),
                "findingsPage": object(&json!({
                    "findings": { "type": "array", "items": { "type": "object" } },
                    "truncated": { "type": "boolean" },
                }), &["findings", "truncated"]),
                "coverage": { "type": "object", "description": "What the review could not check; complete says whether it checked everything." },
            }),
            &["coverage"],
        ),
        "list_findings" => object(
            &json!({
                "entries": { "type": "array", "items": { "type": "object" } },
                "nextCursor": { "type": ["string", "null"], "description": "Pass as cursor for the next page; null at the end." },
                "total": { "type": "integer" },
            }),
            &["entries", "total"],
        ),
        "get_finding" | "update_finding" => {
            object(&json!({ "entry": { "type": "object" } }), &["entry"])
        }
        "explain_finding" => object(
            &json!({
                "finding": { "type": "object" },
                "rule": { "type": "object" },
                "why": any,
                "fix": { "type": "string" },
                "example": any,
                "edit": any,
                "ifYouAcceptIt": { "type": "object" },
            }),
            &["finding", "why", "fix"],
        ),
        "suppress_finding" => object(
            &json!({
                "memoryItemId": { "type": "string" },
                "file": { "type": "string" },
                "ruleId": { "type": "string" },
                "reason": { "type": "string" },
                "effect": { "type": "string" },
            }),
            &["memoryItemId", "reason", "effect"],
        ),
        "list_analyzers" => object(
            &json!({ "analyzers": { "type": "array", "items": { "type": "object" } } }),
            &["analyzers"],
        ),
        "repository_context" => object(&json!({ "context": { "type": "object" } }), &["context"]),
        "repository_summary" => object(&json!({ "summary": { "type": "object" } }), &["summary"]),
        _ => object(&json!({ "findings": {}, "analyzers": {} }), &[]),
    }
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
            description: "Needs the diff text. Reviews the added lines of a unified git diff against the Review Packs that match the repository's stack. Every finding cites the rule that produced it.",
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
            description: "Reviews whole files against the Review Packs that match the repository's stack. Paths must be inside the repository; use review_workspace for everything, or review_changes after an edit.",
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
            description: "Needs a ledgerId from a review result or list_findings; an unknown id fails with \"No finding has that id\" and the way to get one, so list again. Returns everything needed to act on one finding: where it is, why the pattern is a problem, how to fix it (an exact edit when the fix is mechanical, otherwise a before/after example), and what to do if the code is right and the rule is wrong.",
            input: id_only,
            read_only: true,
            destructive: false,
            call: Call::Method("findings/explain", finding_id),
        },
        Tool {
            name: "suppress_finding",
            title: "Accept a finding, with a reason",
            description: "Records that a finding is accepted: every finding of the same rule in the same file stops being reported, and the reason is kept in .debuggatha/memory.json for the team to read. Use it only when the code is right and the rule is wrong for this case, never to make a review quiet. A reason is required (at least 8 characters) and should name the rule and why it does not apply here; it is the audit trail. It changes what later reviews report, so suppress only after explain_finding, and tell the user. An unknown id fails with \"No finding has that id\". Prefer fixing the code; to accept one line instead, add the inline comment explain_finding gives you.",
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
            description: "Returns one ledger entry, including its full lifecycle history. Needs a ledgerId from a review result or list_findings; an unknown id fails with \"No finding has that id\" and the way to get one, so list again.",
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
            description: "Moves a ledger entry to a new lifecycle status and records the change in its history. Needs a ledgerId from a review result or list_findings. A transition the lifecycle does not allow (for example from resolved to acknowledged) is rejected with the reason, and the entry is left as it was; use get_finding to see its state first.",
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
            description: "Reviews the whole repository and records every finding it has today in .debuggatha/baseline.json. It overwrites a baseline already there. From then on, review tools report only findings that are new since the baseline (pass includeBaselined to see all). Use it to adopt Debuggatha on an existing repository without drowning in old findings.",
            input: none,
            read_only: false,
            destructive: true,
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
            "limit",
        ],
    );
    params["scope"] = scope;
    if args.get("analyzers") == Some(&json!(false)) {
        params["analyzers"] = json!({ "mode": "off" });
    }
    params
}
