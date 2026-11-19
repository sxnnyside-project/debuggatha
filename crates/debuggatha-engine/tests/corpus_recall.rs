//! Recall of the detectors, seeded: a known violation in each of several syntactic forms
//! is appended to every clean, non-test JavaScript/TypeScript file of the corpus, and the detector
//! must report it on its own line. This measures whether the surrounding file (strict mode, JSX,
//! decorators, top-level await, a hundred earlier functions) can make the detector miss a
//! violation it would catch alone; it says nothing about forms nobody thought of.
//! Opt-in: `DEBUGGATHA_CORPUS_DIR=<dir> cargo test -p debuggatha-engine --test corpus_recall --release -- --ignored --nocapture`
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::print_stdout)] // a diagnostic test

use std::collections::BTreeMap;
use std::path::Path;

use debuggatha_engine::detect::{DetectorInput, concepts, env, functions, leaks, promises, taint};
use debuggatha_engine::scan_scope::{
    PARITY_EXTENSIONS, is_generated_content, is_test_path, list_scan_files,
};
use debuggatha_engine::syntax::{ParseStatus, parse};

/// (rule, form, source). Each is one line, so the hit must land on the line it was appended at.
const SEEDS: &[(&str, &str, &str)] = &[
    (
        "require-await",
        "declaration",
        "async function seededA() { return 1; }",
    ),
    (
        "require-await",
        "exported declaration",
        "export async function seededB() { return 1; }",
    ),
    (
        "require-await",
        "arrow value",
        "const seededC = async () => 1;",
    ),
    (
        "require-await",
        "arrow block",
        "const seededD = async () => { return 1; };",
    ),
    (
        "require-await",
        "function expression",
        "const seededE = async function () { return 1; };",
    ),
    (
        "require-await",
        "class method",
        "class SeededF { async m() { return 1; } }",
    ),
    (
        "require-await",
        "static method",
        "class SeededG { static async m() { return 1; } }",
    ),
    (
        "require-await",
        "returns a call",
        "async function seededH() { return seededA(); }",
    ),
    (
        "no-loop-func",
        "var counter",
        "for (var seededI = 0; seededI < 3; seededI++) { seededFns.push(function () { return seededI; }); }",
    ),
    (
        "no-loop-func",
        "arrow var counter",
        "for (var seededJ = 0; seededJ < 3; seededJ++) { setTimeout(() => seededLog(seededJ)); }",
    ),
    (
        "no-loop-func",
        "for-in var",
        "for (var seededK in seededObj) { seededCbs.push(() => seededK); }",
    ),
    (
        "no-loop-func",
        "var in body",
        "while (seededMore()) { var seededV = seededNext(); seededCbs.push(() => seededV); }",
    ),
    (
        "no-floating-promises",
        "async function of the file",
        "async function seededL() { return 1; } seededL();",
    ),
    (
        "no-floating-promises",
        "Promise.all",
        "Promise.all([seededM1, seededM2]);",
    ),
    ("no-floating-promises", "fetch", "fetch(seededUrl);"),
    (
        "no-floating-promises",
        "new Promise",
        "new Promise((seededR) => seededR());",
    ),
    (
        "no-floating-promises",
        "then chain",
        "Promise.resolve(1).then(seededCb);",
    ),
    (
        "no-floating-promises",
        "async arrow called at once",
        ";(async () => { await seededWork(); })();",
    ),
    (
        "no-floating-promises",
        "method of the class",
        "class SeededN { async m() { return 1; } run() { this.m(); } }",
    ),
    (
        "no-floating-promises",
        "sleep idiom",
        "function seededSleep(t) { return new Promise((r) => setTimeout(r, t)); } seededSleep(1);",
    ),
    (
        "no-floating-promises",
        "typed Promise function",
        "function seededP(): Promise<void> { return seededQ(); } seededP();",
    ),
    (
        "no-floating-promises",
        "then of unknown origin",
        "seededThing.then(seededCb);",
    ),
    (
        "leak-mock-restore",
        "vi.spyOn",
        "it('seeded', () => { vi.spyOn(seededObj, 'a'); });",
    ),
    (
        "leak-mock-restore",
        "jest.spyOn kept in a variable",
        "it('seeded', () => { const seededSpy = jest.spyOn(seededObj, 'a'); seededSpy.mockReturnValue(1); });",
    ),
    (
        "leak-mock-restore",
        "sinon.stub",
        "it('seeded', () => { sinon.stub(seededObj, 'a'); });",
    ),
    (
        "leak-mock-restore",
        "chained implementation",
        "it('seeded', () => { vi.spyOn(seededObj, 'a').mockImplementation(() => 1); });",
    ),
    (
        "leak-mock-restore",
        "vi.stubGlobal",
        "it('seeded', () => { vi.stubGlobal('seededG', 1); });",
    ),
    (
        "leak-mock-restore",
        "vi.stubEnv",
        "it('seeded', () => { vi.stubEnv('SEEDED', 'v'); });",
    ),
    (
        "leak-mock-restore",
        "process.env assignment",
        "it('seeded', () => { process.env.SEEDED = 'v'; });",
    ),
    (
        "leak-mock-restore",
        "in a beforeEach",
        "beforeEach(() => { vi.spyOn(seededObj, 'a'); });",
    ),
    (
        "n-plus-one-queries",
        "prisma in a for..of",
        "for (const seededU of seededUsers) { await prisma.post.findMany({ where: { id: seededU.id } }); }",
    ),
    (
        "n-plus-one-queries",
        "repository in a for..in",
        "for (const seededK in seededRows) { await seededRepository.findOne(seededK); }",
    ),
    (
        "n-plus-one-queries",
        "map with a query",
        "const seededP = seededUsers.map((seededU) => db.user.findUnique({ where: { id: seededU.id } }));",
    ),
    (
        "n-plus-one-queries",
        "knex chain in forEach",
        "seededUsers.forEach(async (seededU) => { await knex('posts').where({ a: seededU.id }).first(); });",
    ),
    (
        "n-plus-one-queries",
        "ORM model",
        "for (const seededU of seededUsers) { await User.findById(seededU.id); }",
    ),
    (
        "n-plus-one-queries",
        "tagged sql",
        "for (const seededU of seededUsers) { await sql`select 1 where id = ${seededU.id}`; }",
    ),
    (
        "a03-injection",
        "template in a query",
        "function seededA(req, res) { db.query(`SELECT 1 WHERE n = '${req.query.n}'`); }",
    ),
    (
        "a03-injection",
        "concatenation",
        "function seededB(req, res) { db.query(\"SELECT 1 WHERE n = '\" + req.body.n + \"'\"); }",
    ),
    (
        "a03-injection",
        "destructured param",
        "function seededC(req, res) { const { id } = req.params; pool.query(`SELECT 1 WHERE id = ${id}`); }",
    ),
    (
        "a03-injection",
        "unsafe raw",
        "const seededD = async (req, res) => { await prisma.$queryRawUnsafe(`SELECT 1 WHERE n = '${req.query.n}'`); };",
    ),
    (
        "a03-injection",
        "shell command",
        "function seededE(req, res) { const { exec: seededX } = require('child_process'); seededX('ls ' + req.query.d); }",
    ),
    (
        "a03-injection",
        "koa",
        "async function seededF(ctx) { await db.query(`SELECT 1 WHERE n = '${ctx.query.n}'`); }",
    ),
    (
        "a03-injection",
        "hono",
        "app.get('/seeded', async (c) => { await db.query(`SELECT 1 WHERE n = '${c.req.query('n')}'`); });",
    ),
    (
        "a03-injection",
        "whole filter",
        "function seededG(req, res) { User.find(req.query); }",
    ),
    (
        "a03-injection",
        "typeorm where",
        "function seededH(req, res) { qb.where(`u.name = '${req.query.n}'`); }",
    ),
    (
        "validate-input",
        "body",
        "function seededI(req, res) { save(req.body); }",
    ),
    (
        "validate-input",
        "query",
        "function seededJ(req, res) { list(req.query); }",
    ),
    (
        "validate-input",
        "registered route",
        "app.post('/seeded', (req, res) => { save(req.body.name); });",
    ),
    (
        "validate-input",
        "destructured body",
        "function seededK(req, res) { const { name } = req.body; }",
    ),
    (
        "validate-input",
        "controller method",
        "class SeededL { create(req, res) { save(req.body); } }",
    ),
];

#[test]
#[ignore = "needs the corpus clones: run it with its `just corpus-*` recipe"]
fn seeded_violations_are_found_in_real_files() {
    let Some(dir) = std::env::var_os("DEBUGGATHA_CORPUS_DIR") else {
        panic!("DEBUGGATHA_CORPUS_DIR must be set: run it with its `just corpus-*` recipe");
    };
    let mut repos: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    repos.sort();
    let mut table: BTreeMap<(&str, &str), (u32, u32)> = BTreeMap::new(); // found, seeded
    let mut files = 0_u32;
    for repo in repos {
        for file in list_scan_files(&repo, PARITY_EXTENSIONS) {
            let is_js = ["ts", "tsx", "js", "jsx", "mjs", "cjs"]
                .iter()
                .any(|e| Path::new(&file).extension().is_some_and(|x| x == *e));
            if !is_js || is_test_path(&file) || file.ends_with(".d.ts") {
                continue;
            }
            let Ok(bytes) = std::fs::read(repo.join(&file)) else {
                continue;
            };
            let content = String::from_utf8_lossy(&bytes).into_owned();
            if is_generated_content(&content) {
                continue;
            }
            let probe = parse(Path::new(&file), &content).expect("a JS/TS file");
            if probe.status != ParseStatus::Parsed {
                continue;
            }
            files += 1;
            let typed = ["ts", "tsx"]
                .iter()
                .any(|e| Path::new(&file).extension().is_some_and(|x| x == *e));
            // A file with its own `fetch` is not the global one, and the detector must not claim it.
            let own_fetch = [
                "function fetch",
                "fetch =",
                "fetch,",
                "fetch }",
                " fetch from",
            ]
            .iter()
            .any(|d| content.contains(d));
            for (rule, form, seed) in SEEDS {
                if (*form == "typed Promise function" && !typed) || (*form == "fetch" && own_fetch)
                {
                    continue;
                }
                let mut source = content.clone();
                if !source.ends_with('\n') {
                    source.push('\n');
                }
                let line = source.matches('\n').count() + 1;
                source.push_str(seed);
                source.push('\n');
                // A leak is only looked for in a test file: the real file is read under a test name.
                let name = if *rule == "leak-mock-restore" {
                    format!(
                        "{file}.test.{}",
                        Path::new(&file).extension().unwrap().to_str().unwrap()
                    )
                } else {
                    file.clone()
                };
                let syntax = parse(Path::new(&name), &source).expect("a JS/TS file");
                let input = DetectorInput::new(&name, &source, Some(&syntax));
                let hits = if *rule == "n-plus-one-queries" {
                    concepts::detect(&input)
                } else if *rule == "leak-mock-restore" {
                    leaks::detect(&input)
                } else if *rule == "no-floating-promises" {
                    promises::detect(&input)
                } else if *rule == "a03-injection" || *rule == "validate-input" {
                    taint::detect(&input)
                } else {
                    functions::detect(&input)
                };
                let found = hits
                    .iter()
                    .any(|(r, h)| r == rule && h.line as usize == line);
                let row = table.entry((rule, form)).or_default();
                row.1 += 1;
                row.0 += u32::from(found);
            }
        }
    }
    println!("{files} clean non-test files");
    for ((rule, form), (found, seeded)) in &table {
        println!("{rule:<14} {form:<22} {found:>5} / {seeded}");
    }
    for ((rule, form), (found, seeded)) in &table {
        assert_eq!(
            found, seeded,
            "{rule} / {form}: a seeded violation was missed"
        );
    }
}

/// (extensions, form, one-line seed). Every seed reads `SEEDED_ENV_A`, which no file documents.
const ENV_SEEDS: &[(&[&str], &str, &str)] = &[
    (
        &["ts", "tsx", "js", "jsx", "mjs", "cjs"],
        "process.env",
        "const seededEnv = process.env.SEEDED_ENV_A;",
    ),
    (
        &["ts", "tsx", "js", "jsx", "mjs", "cjs"],
        "process.env index",
        "const seededEnv = process.env['SEEDED_ENV_A'];",
    ),
    (
        &["ts", "tsx", "js", "jsx", "mjs", "cjs"],
        "destructuring",
        "const { SEEDED_ENV_A } = process.env;",
    ),
    (
        &["ts", "tsx", "js", "jsx", "mjs", "cjs"],
        "import.meta.env",
        "const seededEnv = import.meta.env.SEEDED_ENV_A;",
    ),
    (
        &["ts", "tsx", "js", "jsx", "mjs", "cjs"],
        "Deno",
        "const seededEnv = Deno.env.get('SEEDED_ENV_A');",
    ),
    (
        &["rs"],
        "std::env::var",
        "pub fn seeded_env() { let _ = std::env::var(\"SEEDED_ENV_A\"); }",
    ),
    (
        &["rs"],
        "env!",
        "pub const SEEDED_ENV: &str = env!(\"SEEDED_ENV_A\");",
    ),
    (
        &["rs"],
        "env! in a macro",
        "pub const SEEDED_ENV: &str = include_str!(env!(\"SEEDED_ENV_A\"));",
    ),
    (
        &["py"],
        "os.getenv",
        "seeded_env = os.getenv(\"SEEDED_ENV_A\")",
    ),
    (
        &["py"],
        "os.environ",
        "seeded_env = os.environ['SEEDED_ENV_A']",
    ),
    (
        &["go"],
        "os.Getenv",
        "var seededEnv = os.Getenv(\"SEEDED_ENV_A\")",
    ),
    (&["php"], "getenv", "$seededEnv = getenv('SEEDED_ENV_A');"),
    (&["php"], "env()", "$seededEnv = env('SEEDED_ENV_A');"),
    (&["php"], "$_ENV", "$seededEnv = $_ENV['SEEDED_ENV_A'];"),
    (
        &["java"],
        "System.getenv",
        "class SeededEnv { String a = System.getenv(\"SEEDED_ENV_A\"); }",
    ),
    (
        &["kt", "kts"],
        "System.getenv",
        "val seededEnv = System.getenv(\"SEEDED_ENV_A\")",
    ),
    (
        &["cs"],
        "Environment",
        "class SeededEnv { string a = Environment.GetEnvironmentVariable(\"SEEDED_ENV_A\"); }",
    ),
    (&["rb"], "ENV[]", "SEEDED_ENV = ENV['SEEDED_ENV_A']"),
    (
        &["rb"],
        "ENV.fetch",
        "SEEDED_ENV = ENV.fetch('SEEDED_ENV_A')",
    ),
    (
        &["swift"],
        "getenv",
        "let seededEnv = getenv(\"SEEDED_ENV_A\")",
    ),
    (
        &["swift"],
        "ProcessInfo",
        "let seededEnv = ProcessInfo.processInfo.environment[\"SEEDED_ENV_A\"]",
    ),
    (
        &["dart"],
        "Platform",
        "final seededEnv = Platform.environment['SEEDED_ENV_A'];",
    ),
    (
        &["dart"],
        "fromEnvironment",
        "final seededEnv = String.fromEnvironment('SEEDED_ENV_A');",
    ),
    (
        &["ex", "exs"],
        "System.get_env",
        "seeded_env = System.get_env(\"SEEDED_ENV_A\")",
    ),
    (
        &["cpp", "cc"],
        "getenv",
        "auto seeded_env = getenv(\"SEEDED_ENV_A\");",
    ),
    (
        &["gd"],
        "OS.get_environment",
        "var seeded_env = OS.get_environment(\"SEEDED_ENV_A\")",
    ),
];

fn files_with_extension(dir: &Path, extensions: &[&str], into: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !matches!(
                name.as_str(),
                "node_modules" | ".git" | "target" | "vendor" | "dist"
            ) {
                files_with_extension(&path, extensions, into);
            }
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| extensions.contains(&e))
        {
            into.push(path);
        }
    }
}

#[test]
#[ignore = "needs the corpus clones: run it with its `just corpus-*` recipe"]
fn a_seeded_environment_read_is_found_in_real_files() {
    let Some(dir) = std::env::var_os("DEBUGGATHA_CORPUS_DIR") else {
        panic!("DEBUGGATHA_CORPUS_DIR must be set: run it with its `just corpus-*` recipe");
    };
    let docs = debuggatha_engine::detect::env_docs::EnvDocs::build(&[]);
    let mut table: BTreeMap<(String, &str), (u32, u32)> = BTreeMap::new();
    for (extensions, form, seed) in ENV_SEEDS {
        let mut files = Vec::new();
        files_with_extension(Path::new(&dir), extensions, &mut files);
        files.sort();
        for path in files {
            let name = path.to_string_lossy().into_owned();
            if is_test_path(&name) || name.ends_with(".d.ts") {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let content = String::from_utf8_lossy(&bytes).into_owned();
            if is_generated_content(&content) {
                continue;
            }
            let Some(probe) = parse(&path, &content) else {
                continue;
            };
            if probe.status != ParseStatus::Parsed {
                continue;
            }
            // Text appended to a template (no `<?php` tag, or HTML after the last `?>`) or after `__END__`
            // is not code.
            let is_php = path.extension().is_some_and(|e| e == "php");
            let is_ruby = path.extension().is_some_and(|e| e == "rb");
            if (is_php
                && (!content.contains("<?php") || content.rfind("?>") > content.rfind("<?php")))
                || (is_ruby && content.contains("\n__END__"))
            {
                continue;
            }
            let mut source = content;
            if !source.ends_with('\n') {
                source.push('\n');
            }
            let line = source.matches('\n').count() + 1;
            source.push_str(seed);
            source.push('\n');
            let syntax = parse(&path, &source).expect("a supported file");
            let input =
                DetectorInput::new(&name, &source, Some(&syntax)).with_env_docs(Some(&docs));
            let found = env::detect(&input)
                .iter()
                .any(|(_, h)| h.line as usize == line);
            let row = table.entry((extensions.join("/"), form)).or_default();
            row.1 += 1;
            row.0 += u32::from(found);
        }
    }
    for ((ext, form), (found, seeded)) in &table {
        println!("{ext:<14} {form:<20} {found:>5} / {seeded}");
    }
    for ((ext, form), (found, seeded)) in &table {
        assert_eq!(found, seeded, "{ext} / {form}: a seeded read was missed");
    }
}
