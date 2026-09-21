/**
 * The labeled set the semantic layer is measured on. Labels are a person's judgment of the
 * code shown, written before any model saw it; where a case is arguable it is not here.
 *
 * Verification cases each contain a line one of Debuggatha's own detectors flags. `truth` says
 * whether that finding is a real problem in this code (`real`) or the code is fine / the rule
 * does not apply (`false_positive`).
 *
 * Detection cases are files with defects planted on known lines (`planted`), and clean files
 * (`planted: []`) where any report is a false alarm.
 */

export type Stack = "ts" | "python" | "rust" | "go";

export interface VerificationCase {
  id: string;
  stack: Stack;
  file: string;
  /** The detector rule that flags the line. */
  rule: string;
  /** A substring of the flagged line, to choose among several findings of the rule. */
  flagged: string;
  truth: "real" | "false_positive";
  code: string;
}

export interface PlantedDefect {
  /** A substring of the line the defect is on. */
  at: string;
  category: "security" | "reliability" | "performance" | "maintainability";
}

export interface DetectionCase {
  id: string;
  stack: Stack;
  file: string;
  planted: PlantedDefect[];
  code: string;
}

export const VERIFICATION: VerificationCase[] = [
  {
    id: "null-assert-find-real",
    stack: "ts",
    file: "src/users.ts",
    rule: "strict-null-checks",
    flagged: "find(",
    truth: "real",
    code: `interface User { id: string; name: string }

export function nameOf(users: User[], id: string): string {
  const user = users.find((u) => u.id === id)!;
  return user.name;
}
`,
  },
  {
    id: "null-assert-header-real",
    stack: "ts",
    file: "src/auth.ts",
    rule: "strict-null-checks",
    flagged: "authorization!",
    truth: "real",
    code: `import type { IncomingMessage } from "node:http";

export function bearerToken(req: IncomingMessage): string {
  const token = req.headers.authorization!.split(" ")[1];
  return token ?? "";
}
`,
  },
  {
    id: "null-assert-checked-index-fp",
    stack: "ts",
    file: "src/first.ts",
    rule: "strict-null-checks",
    flagged: "items[0]!",
    truth: "false_positive",
    code: `export function firstTrimmed(items: string[]): string {
  if (items.length === 0) {
    throw new Error("expected at least one item");
  }
  return items[0]!.trim();
}
`,
  },
  {
    id: "null-assert-checked-el-fp",
    stack: "ts",
    file: "src/main.ts",
    rule: "strict-null-checks",
    flagged: "el!",
    truth: "false_positive",
    code: `import { mount } from "./app";

const el = document.getElementById("root");
if (!el) {
  throw new Error("#root is missing from index.html");
}
mount(el!);
`,
  },
  {
    id: "any-parse-real",
    stack: "ts",
    file: "src/parse.ts",
    rule: "no-explicit-any",
    flagged: "input: any",
    truth: "real",
    code: `export function firstName(input: any): string {
  return input.user.name.first;
}
`,
  },
  {
    id: "any-json-config-real",
    stack: "ts",
    file: "src/config.ts",
    rule: "no-explicit-any",
    flagged: "config: any",
    truth: "real",
    code: `import { readFileSync } from "node:fs";
import { listen } from "./server";

const config: any = JSON.parse(readFileSync("config.json", "utf8"));
listen(config.port, config.host);
`,
  },
  {
    id: "any-never-guard-fp",
    stack: "ts",
    file: "src/exhaustive.ts",
    rule: "no-explicit-any",
    flagged: "as any",
    truth: "false_positive",
    code: `type Shape = { kind: "circle"; r: number } | { kind: "square"; side: number };

export function area(shape: Shape): number {
  switch (shape.kind) {
    case "circle":
      return Math.PI * shape.r ** 2;
    case "square":
      return shape.side ** 2;
    default: {
      // Exhaustiveness check: this branch cannot run, the cast only reads the tag for the message.
      const unreachable: never = shape;
      const kind = (unreachable as any).kind;
      throw new Error("unhandled shape: " + kind);
    }
  }
}
`,
  },
  {
    id: "any-listener-signature-fp",
    stack: "ts",
    file: "src/emitter.ts",
    rule: "no-explicit-any",
    flagged: "args: any[]",
    truth: "false_positive",
    code: `// Listeners take whatever the emitting site passes; \`unknown[]\` would make every handler a type error.
type Listener = (...args: any[]) => void;

export class Emitter {
  private listeners = new Map<string, Listener[]>();

  on(event: string, listener: Listener): void {
    this.listeners.set(event, [...(this.listeners.get(event) ?? []), listener]);
  }

  emit(event: string, ...args: unknown[]): void {
    for (const listener of this.listeners.get(event) ?? []) listener(...args);
  }
}
`,
  },
  {
    id: "eqeq-string-number-real",
    stack: "ts",
    file: "src/paging.ts",
    rule: "eqeqeq",
    flagged: "== 0",
    truth: "real",
    code: `export function pageOf(query: Record<string, string>, firstPage: () => string[]) {
  // query values are always strings
  if (query.page == 0) {
    return firstPage();
  }
  return [];
}
`,
  },
  {
    id: "eqeq-id-coercion-real",
    stack: "ts",
    file: "src/owner.ts",
    rule: "eqeqeq",
    flagged: "!= user.id",
    truth: "real",
    code: `interface User { id: number }

export function canEdit(paramId: string, user: User): boolean {
  // paramId comes from the URL, user.id is a number
  return paramId != user.id ? false : true;
}
`,
  },
  {
    id: "eqeq-typeof-fp",
    stack: "ts",
    file: "src/kind.ts",
    rule: "eqeqeq",
    flagged: "typeof value ==",
    truth: "false_positive",
    code: `export function isText(value: unknown): boolean {
  return typeof value == "string";
}
`,
  },
  {
    id: "eqeq-lengths-fp",
    stack: "ts",
    file: "src/same.ts",
    rule: "eqeqeq",
    flagged: "a.length ==",
    truth: "false_positive",
    code: `export function sameSize(a: string[], b: string[]): boolean {
  return a.length == b.length;
}
`,
  },
  {
    id: "eval-user-input-real",
    stack: "ts",
    file: "src/calc.ts",
    rule: "no-eval",
    flagged: "eval(",
    truth: "real",
    code: `import type { Request, Response } from "express";

export function calculate(req: Request, res: Response) {
  const result = eval(req.body.expression);
  res.json({ result });
}
`,
  },
  {
    id: "html-unsanitized-real",
    stack: "ts",
    file: "src/Comment.tsx",
    rule: "no-dangerously-set-inner-html",
    flagged: "dangerouslySetInnerHTML",
    truth: "real",
    code: `export function Comment({ comment }: { comment: { body: string } }) {
  // comment.body is written by any signed-in user
  return <div dangerouslySetInnerHTML={{ __html: comment.body }} />;
}
`,
  },
  {
    id: "html-sanitized-fp",
    stack: "ts",
    file: "src/Post.tsx",
    rule: "no-dangerously-set-inner-html",
    flagged: "dangerouslySetInnerHTML",
    truth: "false_positive",
    code: `import DOMPurify from "dompurify";

export function Post({ html }: { html: string }) {
  return <article dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(html) }} />;
}
`,
  },
  {
    id: "html-jsonld-fp",
    stack: "ts",
    file: "src/Seo.tsx",
    rule: "no-dangerously-set-inner-html",
    flagged: "dangerouslySetInnerHTML",
    truth: "false_positive",
    code: `const schema = { "@context": "https://schema.org", "@type": "Organization", name: "Acme" };

export function Seo() {
  return (
    <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(schema) }} />
  );
}
`,
  },
  {
    id: "unwrap-read-real",
    stack: "rust",
    file: "src/load.rs",
    rule: "no-unwrap-expect",
    flagged: "read_to_string",
    truth: "real",
    code: `use std::fs;

/// \`path\` is chosen by the user on the command line.
pub fn load(path: &str) -> String {
    fs::read_to_string(path).unwrap()
}
`,
  },
  {
    id: "unwrap-const-regex-fp",
    stack: "rust",
    file: "src/slug.rs",
    rule: "no-unwrap-expect",
    flagged: "Regex::new",
    truth: "false_positive",
    code: `use regex::Regex;
use std::sync::OnceLock;

pub fn is_slug(text: &str) -> bool {
    static SLUG: OnceLock<Regex> = OnceLock::new();
    let re = SLUG.get_or_init(|| Regex::new(r"^[a-z0-9-]+$").unwrap());
    re.is_match(text)
}
`,
  },
  {
    id: "expect-checked-fp",
    stack: "rust",
    file: "src/head.rs",
    rule: "no-unwrap-expect",
    flagged: "expect(",
    truth: "false_positive",
    code: `pub fn head(names: &[String]) -> Option<&str> {
    if names.is_empty() {
        return None;
    }
    let first = names.first().expect("non-empty checked above");
    Some(first.as_str())
}
`,
  },
  {
    id: "unsafe-unchecked-index-real",
    stack: "rust",
    file: "src/read.rs",
    rule: "no-unsafe-blocks",
    flagged: "unsafe",
    truth: "real",
    code: `/// \`offset\` comes straight from the request body.
pub fn byte_at(buf: &[u8], offset: usize) -> u8 {
    unsafe { *buf.as_ptr().add(offset) }
}
`,
  },
  {
    id: "unsafe-validated-utf8-fp",
    stack: "rust",
    file: "src/text.rs",
    rule: "no-unsafe-blocks",
    flagged: "unsafe",
    truth: "false_positive",
    code: `pub fn as_str(buf: &[u8]) -> Option<&str> {
    if std::str::from_utf8(buf).is_err() {
        return None;
    }
    // SAFETY: the bytes were just validated as UTF-8 above.
    Some(unsafe { std::str::from_utf8_unchecked(buf) })
}
`,
  },
  {
    id: "panic-user-command-real",
    stack: "rust",
    file: "src/command.rs",
    rule: "no-panic",
    flagged: "panic!",
    truth: "real",
    code: `pub enum Command { Start, Stop }

/// \`word\` is text typed by a remote client.
pub fn parse(word: &str) -> Command {
    match word {
        "start" => Command::Start,
        "stop" => Command::Stop,
        other => panic!("unsupported command: {}", other),
    }
}
`,
  },
  {
    id: "panic-startup-config-fp",
    stack: "rust",
    file: "src/main.rs",
    rule: "no-panic",
    flagged: "panic!",
    truth: "false_positive",
    code: `fn main() {
    // Fail fast at startup: the service cannot do anything useful without a database.
    let url = match std::env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => panic!("DATABASE_URL must be set"),
    };
    run(&url);
}

fn run(_url: &str) {}
`,
  },
  {
    id: "panic-handler-real",
    stack: "go",
    file: "handler.go",
    rule: "no-panic",
    flagged: "panic(err)",
    truth: "real",
    code: `package main

import (
	"database/sql"
	"net/http"
)

func handle(db *sql.DB) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		row := db.QueryRow("SELECT name FROM users WHERE id = $1", r.URL.Query().Get("id"))
		var name string
		if err := row.Scan(&name); err != nil {
			panic(err)
		}
		w.Write([]byte(name))
	}
}
`,
  },
  {
    id: "panic-must-helper-fp",
    stack: "go",
    file: "must.go",
    rule: "no-panic",
    flagged: "panic(",
    truth: "false_positive",
    code: `package main

import "regexp"

// MustCompile is used only for package-level patterns that are constants in the source.
func mustCompile(pattern string) *regexp.Regexp {
	re, err := regexp.Compile(pattern)
	if err != nil {
		panic("bad built-in pattern " + pattern + ": " + err.Error())
	}
	return re
}

var slug = mustCompile("^[a-z0-9-]+$")
`,
  },
];

export const DETECTION: DetectionCase[] = [
  {
    id: "off-by-one",
    stack: "ts",
    file: "src/total.ts",
    planted: [{ at: "i <= items.length", category: "reliability" }],
    code: `export function total(items: { price: number }[]): number {
  let sum = 0;
  for (let i = 0; i <= items.length; i++) {
    sum += items[i].price;
  }
  return sum;
}
`,
  },
  {
    id: "sql-concat",
    stack: "ts",
    file: "src/orders.ts",
    planted: [{ at: "DELETE FROM orders", category: "security" }],
    code: `interface Db { query(sql: string, params?: unknown[]): Promise<unknown> }

export async function removeOrder(db: Db, req: { params: { id: string } }) {
  await db.query("DELETE FROM orders WHERE id = " + req.params.id);
}
`,
  },
  {
    id: "missing-authorization",
    stack: "ts",
    file: "src/documents.ts",
    planted: [{ at: "documents.delete(", category: "security" }],
    code: `interface Doc { id: string; ownerId: string }
interface Store { get(id: string): Doc | undefined; delete(id: string): void }

export function deleteDocument(documents: Store, currentUserId: string, id: string) {
  const doc = documents.get(id);
  if (!doc) return { status: 404 };
  documents.delete(id);
  return { status: 204, deletedBy: currentUserId };
}
`,
  },
  {
    id: "missing-await",
    stack: "ts",
    file: "src/checkout.ts",
    planted: [{ at: "orders.save(order);", category: "reliability" }],
    code: `interface Orders { save(order: object): Promise<void> }

export async function checkout(orders: Orders, order: object) {
  orders.save(order);
  return { ok: true };
}
`,
  },
  {
    id: "division-by-zero",
    stack: "ts",
    file: "src/stats.ts",
    planted: [{ at: "total / scores.length", category: "reliability" }],
    code: `export function average(scores: number[]): number {
  const total = scores.reduce((sum, score) => sum + score, 0);
  return total / scores.length;
}
`,
  },
  {
    id: "jwt-decode-only",
    stack: "ts",
    file: "src/session.ts",
    planted: [{ at: "jwt.decode(", category: "security" }],
    code: `import jwt from "jsonwebtoken";

export function currentUser(token: string): string | undefined {
  const payload = jwt.decode(token) as { sub?: string } | null;
  return payload?.sub;
}
`,
  },
  {
    id: "path-traversal",
    stack: "python",
    file: "download.py",
    planted: [{ at: "open(os.path.join", category: "security" }],
    code: `import os
from flask import Flask, request

app = Flask(__name__)
BASE = "/srv/files"


@app.route("/download")
def download():
    name = request.args["file"]
    with open(os.path.join(BASE, name), "rb") as handle:
        return handle.read()
`,
  },
  {
    id: "clean-slugify",
    stack: "ts",
    file: "src/slugify.ts",
    planted: [],
    code: `export function slugify(text: string): string {
  return text
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}
`,
  },
  {
    id: "clean-average",
    stack: "ts",
    file: "src/safe-stats.ts",
    planted: [],
    code: `export function average(scores: number[]): number | undefined {
  if (scores.length === 0) return undefined;
  const total = scores.reduce((sum, score) => sum + score, 0);
  return total / scores.length;
}
`,
  },
  {
    id: "clean-download",
    stack: "python",
    file: "safe_download.py",
    planted: [],
    code: `import os
from flask import Flask, abort, request

app = Flask(__name__)
BASE = os.path.realpath("/srv/files")


@app.route("/download")
def download():
    path = os.path.realpath(os.path.join(BASE, request.args["file"]))
    if os.path.commonpath([BASE, path]) != BASE or not os.path.isfile(path):
        abort(404)
    with open(path, "rb") as handle:
        return handle.read()
`,
  },
];
