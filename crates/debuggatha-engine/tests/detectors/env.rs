//! `document-env-vars` is specified by the cases below. They say which files
//! document configuration and what counts as a mention (`env_docs`), which calls read the
//! environment in each language and which look-alikes do not (`env::reads_of`), and what the rule
//! reports once the two meet. Mutations of the detector must fail at least one.
#![allow(clippy::expect_used, clippy::unwrap_used)]
// a failed expect is a failed test
// The tests are tables of cases: splitting a table only hides what it covers.
#![allow(clippy::too_many_lines)]

use std::path::Path;

use debuggatha_core::env::Env;
use debuggatha_core::knowledge_system::ReviewPolicy;
use debuggatha_engine::detect::coverage::GapReason;
use debuggatha_engine::detect::rules::env;
use debuggatha_engine::detect::rules::env_docs::{DocKind, EnvDocs, doc_kind};
use debuggatha_engine::detect::{
    DetectorInput, RepoFile, SourceUnit, default_registry, run_review, run_review_with_files,
};
use debuggatha_engine::syntax::parse;
use serde_json::{Value, json};

fn file(path: &str, content: &str) -> RepoFile {
    RepoFile {
        path: path.into(),
        content: content.into(),
    }
}

fn reads(file: &str, source: &str) -> Vec<String> {
    let syntax = parse(Path::new(file), source).expect("a supported language");
    let input = DetectorInput::new(file, source, Some(&syntax));
    env::reads_of(&input).into_iter().map(|r| r.name).collect()
}

fn check_reads(cases: &[(&str, &str, &str, &[&str])]) {
    let mut bad = Vec::new();
    for (name, path, source, want) in cases {
        let got = reads(path, source);
        if got != *want {
            bad.push(format!("{name}: got {got:?}, want {want:?}"));
        }
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

#[test]
fn every_query_compiles() {
    assert_eq!(env::compile_errors(), Vec::<String>::new());
}

#[test]
fn javascript_and_typescript() {
    check_reads(&[
        (
            "process.env member",
            "a.ts",
            "const a = process.env.A;\n",
            &["A"],
        ),
        (
            "process.env index",
            "a.ts",
            "const a = process.env['B'] + process.env[\"C\"];\n",
            &["B", "C"],
        ),
        (
            "import.meta.env",
            "a.ts",
            "const a = import.meta.env.VITE_D;\n",
            &["VITE_D"],
        ),
        ("Bun.env", "a.ts", "const a = Bun.env.E;\n", &["E"]),
        (
            "Deno.env.get",
            "a.ts",
            "const a = Deno.env.get('F');\n",
            &["F"],
        ),
        (
            "destructuring",
            "a.ts",
            "const { G, H: h } = process.env;\n",
            &["G", "H"],
        ),
        (
            "optional chain",
            "a.ts",
            "const a = process.env?.I;\n",
            &["I"],
        ),
        (
            "a default",
            "a.js",
            "const port = process.env.PORT || 3000;\n",
            &["PORT"],
        ),
        (
            "in a function",
            "a.js",
            "function f() { return process.env.J; }\n",
            &["J"],
        ),
        (
            "a jsx file",
            "a.jsx",
            "const a = <p>{process.env.K}</p>;\n",
            &["K"],
        ),
        (
            "source order",
            "a.ts",
            "const b = process.env.Z;\nconst a = process.env.A;\n",
            &["Z", "A"],
        ),
        (
            "a computed name is not a name",
            "a.ts",
            "const a = process.env[name];\n",
            &[],
        ),
        ("a write", "a.ts", "process.env.A = 'x';\n", &[]),
        ("a logical write", "a.ts", "process.env.A ||= 'x';\n", &[]),
        ("a delete", "a.ts", "delete process.env.A;\n", &[]),
        (
            "the whole object",
            "a.ts",
            "const e = process.env;\nconsole.log(e);\n",
            &[],
        ),
        (
            "another object called env",
            "a.ts",
            "const a = config.env.A;\n",
            &[],
        ),
        ("another env", "a.ts", "const a = process.argv.A;\n", &[]),
        ("in a comment", "a.ts", "// process.env.A\n", &[]),
        ("in a string", "a.ts", "const s = 'process.env.A';\n", &[]),
        (
            "Deno.env.set is a write",
            "a.ts",
            "Deno.env.set('A', 'x');\n",
            &[],
        ),
        (
            "Deno.env.get with a variable",
            "a.ts",
            "Deno.env.get(name);\n",
            &[],
        ),
        ("another get", "a.ts", "map.get('A');\n", &[]),
        (
            "a write and a read",
            "a.ts",
            "process.env.A = process.env.B;\n",
            &["B"],
        ),
    ]);
}

#[test]
fn rust() {
    check_reads(&[
        (
            "std::env::var",
            "a.rs",
            "fn f() { let a = std::env::var(\"A\"); }\n",
            &["A"],
        ),
        (
            "env::var",
            "a.rs",
            "fn f() { let a = env::var(\"B\"); }\n",
            &["B"],
        ),
        (
            "var_os",
            "a.rs",
            "fn f() { let a = env::var_os(\"C\"); }\n",
            &["C"],
        ),
        ("env!", "a.rs", "const A: &str = env!(\"D\");\n", &["D"]),
        (
            "option_env!",
            "a.rs",
            "const A: Option<&str> = option_env!(\"E\");\n",
            &["E"],
        ),
        (
            "a leading path",
            "a.rs",
            "fn f() { let a = ::std::env::var(\"F\"); }\n",
            &["F"],
        ),
        (
            "a variable name",
            "a.rs",
            "fn f(n: &str) { let a = env::var(n); }\n",
            &[],
        ),
        (
            "another var function",
            "a.rs",
            "fn f() { let a = other::var(\"A\"); }\n",
            &[],
        ),
        (
            "a method called var",
            "a.rs",
            "fn f() { let a = x.var(\"A\"); }\n",
            &[],
        ),
        (
            "a computed name",
            "a.rs",
            "fn f() { let a = env::var(&format!(\"A{}\", 1)); }\n",
            &[],
        ),
        (
            "in a string",
            "a.rs",
            "const S: &str = \"env::var(\\\"A\\\")\";\n",
            &[],
        ),
        (
            "in a comment",
            "a.rs",
            "// env::var(\"A\")\nfn f() {}\n",
            &[],
        ),
        (
            "env::vars",
            "a.rs",
            "fn f() { for _ in env::vars() {} }\n",
            &[],
        ),
        (
            "a macro that is not env",
            "a.rs",
            "fn f() { let a = format!(\"A\"); }\n",
            &[],
        ),
    ]);
}

#[test]
fn python() {
    check_reads(&[
        ("os.getenv", "a.py", "a = os.getenv('A')\n", &["A"]),
        (
            "with a default",
            "a.py",
            "a = os.getenv(\"B\", \"x\")\n",
            &["B"],
        ),
        ("os.environ index", "a.py", "a = os.environ['C']\n", &["C"]),
        (
            "os.environ.get",
            "a.py",
            "a = os.environ.get('D')\n",
            &["D"],
        ),
        ("imported environ", "a.py", "a = environ.get('E')\n", &["E"]),
        ("imported getenv", "a.py", "a = getenv('F')\n", &["F"]),
        ("a write", "a.py", "os.environ['A'] = '1'\n", &[]),
        (
            "a write and a read",
            "a.py",
            "os.environ['A'] = os.environ['B']\n",
            &["B"],
        ),
        ("a variable name", "a.py", "a = os.getenv(name)\n", &[]),
        ("an f-string", "a.py", "a = os.getenv(f'A_{x}')\n", &[]),
        ("another get", "a.py", "a = config.get('A')\n", &[]),
        ("another index", "a.py", "a = settings['A']\n", &[]),
        (
            "a name with a space",
            "a.py",
            "a = os.getenv('not a name')\n",
            &[],
        ),
        ("an empty name", "a.py", "a = os.getenv('')\n", &[]),
        ("in a comment", "a.py", "# os.getenv('A')\n", &[]),
        (
            "in a docstring",
            "a.py",
            "\"\"\"os.getenv('A')\"\"\"\n",
            &[],
        ),
    ]);
}

#[test]
fn go() {
    check_reads(&[
        (
            "Getenv",
            "a.go",
            "package m\nfunc f() { a := os.Getenv(\"A\") }\n",
            &["A"],
        ),
        (
            "LookupEnv",
            "a.go",
            "package m\nfunc f() { a, ok := os.LookupEnv(\"B\") }\n",
            &["B"],
        ),
        (
            "a raw string",
            "a.go",
            "package m\nfunc f() { a := os.Getenv(`C`) }\n",
            &["C"],
        ),
        (
            "Setenv is a write",
            "a.go",
            "package m\nfunc f() { os.Setenv(\"A\", \"b\") }\n",
            &[],
        ),
        (
            "a variable name",
            "a.go",
            "package m\nfunc f(n string) { a := os.Getenv(n) }\n",
            &[],
        ),
        (
            "another package",
            "a.go",
            "package m\nfunc f() { a := viper.GetString(\"a\") }\n",
            &[],
        ),
    ]);
}

#[test]
fn php() {
    check_reads(&[
        ("getenv", "a.php", "<?php $a = getenv('A');\n", &["A"]),
        ("env helper", "a.php", "<?php $a = env('B', 'x');\n", &["B"]),
        ("$_ENV", "a.php", "<?php $a = $_ENV['C'];\n", &["C"]),
        (
            "double quotes",
            "a.php",
            "<?php $a = getenv(\"D\");\n",
            &["D"],
        ),
        ("a write", "a.php", "<?php $_ENV['A'] = 1;\n", &[]),
        (
            "$_SERVER is request data",
            "a.php",
            "<?php $a = $_SERVER['REQUEST_METHOD'];\n",
            &[],
        ),
        (
            "config helper",
            "a.php",
            "<?php $a = config('app.name');\n",
            &[],
        ),
        ("a variable name", "a.php", "<?php $a = getenv($n);\n", &[]),
        (
            "a method named env",
            "a.php",
            "<?php $a = $x->env('A');\n",
            &[],
        ),
    ]);
}

#[test]
fn java_kotlin_and_csharp() {
    check_reads(&[
        (
            "java",
            "A.java",
            "class A { void f() { String a = System.getenv(\"A\"); } }\n",
            &["A"],
        ),
        (
            "java getProperty",
            "A.java",
            "class A { void f() { String a = System.getProperty(\"a\"); } }\n",
            &[],
        ),
        (
            "java all",
            "A.java",
            "class A { void f() { var a = System.getenv(); } }\n",
            &[],
        ),
        (
            "java variable",
            "A.java",
            "class A { void f(String n) { var a = System.getenv(n); } }\n",
            &[],
        ),
        (
            "kotlin",
            "a.kt",
            "fun f() { val a = System.getenv(\"B\") }\n",
            &["B"],
        ),
        (
            "kotlin other",
            "a.kt",
            "fun f() { val a = props.getProperty(\"b\") }\n",
            &[],
        ),
        (
            "csharp",
            "A.cs",
            "class A { void F() { var a = Environment.GetEnvironmentVariable(\"C\"); } }\n",
            &["C"],
        ),
        (
            "csharp qualified",
            "A.cs",
            "class A { void F() { var a = System.Environment.GetEnvironmentVariable(\"D\"); } }\n",
            &["D"],
        ),
        (
            "csharp configuration",
            "A.cs",
            "class A { void F() { var a = Configuration[\"E\"]; } }\n",
            &[],
        ),
    ]);
}

#[test]
fn ruby_swift_dart_elixir_cpp_gdscript() {
    check_reads(&[
        ("ruby index", "a.rb", "a = ENV['A']\n", &["A"]),
        ("ruby fetch", "a.rb", "a = ENV.fetch('B', 'x')\n", &["B"]),
        ("ruby write", "a.rb", "ENV['A'] = '1'\n", &[]),
        ("ruby key?", "a.rb", "a = ENV.key?('A')\n", &[]),
        ("ruby params", "a.rb", "a = params['A']\n", &[]),
        ("ruby interpolated", "a.rb", "a = ENV[\"A#{x}\"]\n", &[]),
        ("swift getenv", "a.swift", "let a = getenv(\"A\")\n", &["A"]),
        (
            "swift ProcessInfo",
            "a.swift",
            "let a = ProcessInfo.processInfo.environment[\"B\"]\n",
            &["B"],
        ),
        (
            "swift defaults",
            "a.swift",
            "let a = UserDefaults.standard.string(forKey: \"A\")\n",
            &[],
        ),
        (
            "dart Platform",
            "a.dart",
            "void f() { var a = Platform.environment['A']; }\n",
            &["A"],
        ),
        (
            "dart fromEnvironment",
            "a.dart",
            "void f() { var a = String.fromEnvironment('B'); }\n",
            &["B"],
        ),
        (
            "dart const",
            "a.dart",
            "void f() { var a = const String.fromEnvironment('C', defaultValue: 'x'); }\n",
            &["C"],
        ),
        (
            "dart bool",
            "a.dart",
            "void f() { var a = bool.fromEnvironment('D'); }\n",
            &["D"],
        ),
        (
            "dart map",
            "a.dart",
            "void f() { var a = map['A']; }\n",
            &[],
        ),
        (
            "elixir get_env",
            "a.ex",
            "a = System.get_env(\"A\")\n",
            &["A"],
        ),
        (
            "elixir fetch_env!",
            "a.ex",
            "a = System.fetch_env!(\"B\")\n",
            &["B"],
        ),
        (
            "elixir default",
            "a.ex",
            "a = System.get_env(\"C\", \"x\")\n",
            &["C"],
        ),
        (
            "elixir put_env",
            "a.ex",
            "System.put_env(\"A\", \"b\")\n",
            &[],
        ),
        (
            "elixir Application",
            "a.ex",
            "a = Application.get_env(:app, :a)\n",
            &[],
        ),
        (
            "cpp getenv",
            "a.cpp",
            "void f() { auto a = getenv(\"A\"); }\n",
            &["A"],
        ),
        (
            "cpp std::getenv",
            "a.cpp",
            "void f() { auto a = std::getenv(\"B\"); }\n",
            &["B"],
        ),
        (
            "cpp variable",
            "a.cpp",
            "void f(const char* n) { auto a = getenv(n); }\n",
            &[],
        ),
        (
            "gdscript",
            "a.gd",
            "func f():\n\tvar a = OS.get_environment(\"A\")\n",
            &["A"],
        ),
        (
            "gdscript other",
            "a.gd",
            "func f():\n\tvar a = OS.get_name()\n",
            &[],
        ),
    ]);
}

#[test]
fn html_is_not_read() {
    check_reads(&[("html", "a.html", "<p>process.env.A</p>\n", &[])]);
}

#[test]
fn which_files_document_configuration() {
    use DocKind::{Config, Dockerfile, Dotenv, Markdown};
    let cases: &[(&str, Option<DocKind>)] = &[
        (".env", Some(Dotenv)),
        (".env.example", Some(Dotenv)),
        (".env.production", Some(Dotenv)),
        ("config/.env.sample", Some(Dotenv)),
        ("example.env", Some(Dotenv)),
        ("env.example", Some(Dotenv)),
        (".envrc", Some(Dotenv)),
        ("README.md", Some(Markdown)),
        ("README", Some(Markdown)),
        ("README.rst", Some(Markdown)),
        ("docs/usage.adoc", Some(Markdown)),
        ("requirements.txt", None),
        ("docs/setup.md", Some(Markdown)),
        ("Dockerfile", Some(Dockerfile)),
        ("services/api/Dockerfile.dev", Some(Dockerfile)),
        ("api.dockerfile", Some(Dockerfile)),
        ("Containerfile", Some(Dockerfile)),
        ("docker-compose.yml", Some(Config)),
        ("docker-compose.override.yaml", Some(Config)),
        ("compose.yaml", Some(Config)),
        (".github/workflows/ci.yml", Some(Config)),
        (".gitlab-ci.yml", Some(Config)),
        ("Makefile", Some(Config)),
        ("justfile", Some(Config)),
        ("Justfile", Some(Config)),
        ("package.json", Some(Config)),
        ("composer.json", Some(Config)),
        ("pyproject.toml", Some(Config)),
        (".cargo/config.toml", Some(Config)),
        ("fly.toml", Some(Config)),
        ("wrangler.toml", Some(Config)),
        ("Procfile", Some(Config)),
        ("src/main.rs", None),
        ("src/env.ts", None),
        (".env.ts", None),
        ("app.env.ts", None),
        ("config/env.js", None),
        ("prod.env", Some(Dotenv)),
        (".env.production.local", Some(Dotenv)),
        ("config.yaml", None),
        ("settings.yml", None),
        ("Cargo.toml", None),
        ("tsconfig.json", None),
        ("src\\app\\.env.example", Some(Dotenv)),
    ];
    let bad: Vec<_> = cases
        .iter()
        .filter(|(path, want)| doc_kind(path) != *want)
        .map(|(path, want)| format!("{path}: got {:?}, want {want:?}", doc_kind(path)))
        .collect();
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

fn mentions(path: &str, content: &str, yes: &[&str], no: &[&str]) {
    let docs = EnvDocs::build(&[file(path, content)]);
    for n in yes {
        assert!(
            docs.documents(n),
            "{path}: {n} should be mentioned; has {:?}",
            docs.names().collect::<Vec<_>>()
        );
    }
    for n in no {
        assert!(!docs.documents(n), "{path}: {n} should not be mentioned");
    }
}

#[test]
fn what_a_file_mentions() {
    mentions(
        ".env.example",
        "# the database\nDATABASE_URL=postgres://x\nexport API_KEY=abc\n# OPTIONAL_FLAG=1\n  SPACED = 2\nnot a var\nlower=1\nA==b\n",
        &[
            "DATABASE_URL",
            "API_KEY",
            "OPTIONAL_FLAG",
            "SPACED",
            "lower",
            "A",
        ],
        &["not", "the", "database", "postgres"],
    );
    mentions(
        "README.md",
        "Set PORT before you start.\n\nThe `API_URL` variable and `npm install` and `lower`.\n\n```sh\nexport TOKEN=abc\nDEBUG\nhelp\n```\n\n~~~\nTILDE\n~~~\n\nAfter the fence: OUTSIDE_FENCE\n",
        &["API_URL", "TOKEN", "DEBUG", "TILDE"],
        &["PORT", "OUTSIDE_FENCE", "npm", "install", "lower", "help"],
    );
    mentions(
        "docs/usage.rst",
        "Set ``REQUESTS_CA_BUNDLE`` and `SINGLE_TICK` but not ``lower`` or plain TEXT_WORD.\n",
        &["REQUESTS_CA_BUNDLE", "SINGLE_TICK"],
        &["lower", "TEXT_WORD"],
    );
    mentions(
        "Dockerfile",
        "FROM x\nENV A=1 B=\"two words\" \\\n  C=3\nENV D value\nARG E\nARG F=default\nRUN echo $G ${H:-x}\n# ENV Z=1\nenv LOWER_KEYWORD=1\n",
        &["A", "B", "C", "D", "E", "F", "G", "H", "LOWER_KEYWORD"],
        &["Z", "two", "words", "value"],
    );
    mentions(
        "docker-compose.yml",
        "services:\n  web:\n    environment:\n      API_KEY: abc\n      DB_HOST: \"db\"\n    command: run ${PORT:-8080}\n    ports:\n      - '8080:80'\n  worker:\n    environment:\n      - REDIS_URL\n      - QUEUE=default\n    image: http://host:8080/path\n",
        &["API_KEY", "DB_HOST", "PORT", "REDIS_URL", "QUEUE"],
        &["host", "path", "8080"],
    );
    mentions(
        ".github/workflows/ci.yml",
        "env:\n  NODE_VERSION: 20\njobs:\n  test:\n    steps:\n      - run: echo ${{ secrets.TOKEN }} $HOME_DIR\n",
        &["NODE_VERSION", "HOME_DIR"],
        &["TOKEN"],
    );
    mentions(
        "package.json",
        "{\n  \"scripts\": {\n    \"dev\": \"PORT=3000 node server.js\",\n    \"start\": \"node $APP_ENTRY\"\n  },\n  \"env\": {\n    \"API_BASE\": \"x\"\n  }\n}\n",
        &["PORT", "APP_ENTRY", "API_BASE"],
        &["server"],
    );
    mentions(
        "composer.json",
        "{\"scripts\": {\"post-install\": \"APP_ENV=local php artisan key:generate\"}}\n",
        &["APP_ENV"],
        &["artisan"],
    );
    mentions(
        "Makefile",
        "API_URL ?= http://x\nexport DEBUG := 1\nLEVEL = 3\nbuild:\n\tcc -o app $(CFLAGS) $$HOME\n",
        &["API_URL", "DEBUG", "LEVEL"],
        &["cc", "app"],
    );
    mentions(
        "justfile",
        "export DATABASE_URL := \"x\"\nset dotenv-load\nrun:\n    cargo run\n",
        &["DATABASE_URL"],
        &["cargo", "run"],
    );
    mentions(
        ".cargo/config.toml",
        "[env]\nRUST_FOO = \"1\"\nBAR = { value = \"x\", force = true }\n",
        &["RUST_FOO", "BAR"],
        &["value", "force"],
    );
    mentions(
        "pyproject.toml",
        "[tool.pytest.ini_options]\nenv = [\"DJANGO_SETTINGS_MODULE=app.settings\"]\n",
        &["DJANGO_SETTINGS_MODULE"],
        &["app"],
    );
    mentions(
        "fly.toml",
        "[env]\n  PRIMARY_REGION = \"gru\"\n",
        &["PRIMARY_REGION"],
        &[],
    );
    // A file that is not documentation is not read, whatever it says.
    let docs = EnvDocs::build(&[file("src/main.rs", "A=1\n"), file("Cargo.toml", "B=1\n")]);
    assert_eq!(docs.files_read(), 0);
    assert!(!docs.documents("A") && !docs.documents("B"));
}

fn detect(path: &str, source: &str, files: Option<&[RepoFile]>) -> Vec<(u32, u32, String)> {
    let syntax = parse(Path::new(path), source).expect("a supported language");
    let docs = files.map(EnvDocs::build);
    let input = DetectorInput::new(path, source, Some(&syntax)).with_env_docs(docs.as_ref());
    env::detect(&input)
        .into_iter()
        .map(|(rule, h)| {
            assert_eq!(rule, "document-env-vars");
            (
                h.line,
                h.columns.start,
                serde_json::to_value(h.confidence)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string(),
            )
        })
        .collect()
}

#[test]
fn an_undocumented_variable_is_reported_in_any_language() {
    let none: &[RepoFile] = &[];
    let cases: &[(&str, &str, &str, u32)] = &[
        ("js", "a.js", "const a = process.env.SECRET_A;\n", 11),
        (
            "ts",
            "a.ts",
            "const x = 1;\nconst a = import.meta.env.VITE_A;\n",
            11,
        ),
        (
            "rust",
            "a.rs",
            "fn f() { let a = std::env::var(\"SECRET_A\"); }\n",
            32,
        ),
        ("python", "a.py", "x = os.getenv(\"SECRET_A\")\n", 15),
        (
            "go",
            "a.go",
            "package m\nfunc f() { a := os.Getenv(\"SECRET_A\") }\n",
            27,
        ),
        ("php", "a.php", "<?php $a = getenv('SECRET_A');\n", 19),
        (
            "java",
            "A.java",
            "class A { String a = System.getenv(\"SECRET_A\"); }\n",
            36,
        ),
        (
            "kotlin",
            "a.kt",
            "val a = System.getenv(\"SECRET_A\")\n",
            23,
        ),
        (
            "csharp",
            "A.cs",
            "class A { string a = Environment.GetEnvironmentVariable(\"SECRET_A\"); }\n",
            57,
        ),
        ("ruby", "a.rb", "a = ENV['SECRET_A']\n", 9),
        ("swift", "a.swift", "let a = getenv(\"SECRET_A\")\n", 16),
        (
            "dart",
            "a.dart",
            "var a = Platform.environment['SECRET_A'];\n",
            30,
        ),
        ("elixir", "a.ex", "a = System.get_env(\"SECRET_A\")\n", 20),
        ("cpp", "a.cpp", "auto a = getenv(\"SECRET_A\");\n", 17),
        (
            "gdscript",
            "a.gd",
            "func f():\n\tvar a = OS.get_environment(\"SECRET_A\")\n",
            29,
        ),
    ];
    for (name, path, source, column) in cases {
        let got = detect(path, source, Some(none));
        let line = u32::try_from(source.trim_end().matches('\n').count()).unwrap() + 1;
        assert_eq!(
            got,
            vec![(line, *column, "medium".to_string())],
            "{name}: {got:?}"
        );
    }
}

#[test]
fn a_documented_variable_is_not() {
    let py = "x = os.getenv(\"SECRET_A\")\n";
    let docs: &[(&str, &[RepoFile])] = &[
        ("dotenv", &[file(".env.example", "SECRET_A=\n")]),
        ("readme", &[file("README.md", "Set `SECRET_A` first.\n")]),
        ("dockerfile", &[file("Dockerfile", "ENV SECRET_A=1\n")]),
        (
            "compose",
            &[file(
                "docker-compose.yml",
                "services:\n  a:\n    environment:\n      - SECRET_A\n",
            )],
        ),
        (
            "workflow",
            &[file(".github/workflows/ci.yml", "env:\n  SECRET_A: x\n")],
        ),
        (
            "package manager",
            &[file(
                "package.json",
                "{\"scripts\":{\"start\":\"SECRET_A=1 node .\"}}",
            )],
        ),
        ("makefile", &[file("Makefile", "SECRET_A ?= 1\n")]),
    ];
    for (name, files) in docs {
        assert_eq!(detect("a.py", py, Some(files)), vec![], "{name}");
    }
    // Prose is not a mention: the name written bare outside code does not document anything.
    assert_eq!(
        detect(
            "a.py",
            py,
            Some(&[file("README.md", "Set SECRET_A first.\n")])
        )
        .len(),
        1
    );
    // Another variable's documentation does not cover this one.
    assert_eq!(
        detect("a.py", py, Some(&[file(".env.example", "OTHER=1\n")])).len(),
        1
    );
    // Names are case-sensitive, as in the environment.
    assert_eq!(
        detect("a.py", py, Some(&[file(".env.example", "secret_a=1\n")])).len(),
        1
    );
}

#[test]
fn what_the_rule_leaves_alone() {
    let none: &[RepoFile] = &[];
    // The platform's own variables.
    for name in [
        "PATH",
        "HOME",
        "CI",
        "NODE_ENV",
        "GITHUB_TOKEN",
        "CARGO_MANIFEST_DIR",
        "OTEL_EXPORTER_OTLP_PROTOCOL",
        "XDG_CONFIG_HOME",
        "LC_ALL",
        "OUT_DIR",
    ] {
        let source = format!("x = os.getenv(\"{name}\")\n");
        assert_eq!(detect("a.py", &source, Some(none)), vec![], "{name}");
    }
    // Without the repository's files the rule cannot conclude, so it says nothing.
    assert_eq!(
        detect("a.py", "x = os.getenv(\"SECRET_A\")\n", None),
        vec![]
    );
    // A test sets the variables it needs.
    assert_eq!(
        detect("a_test.py", "x = os.getenv(\"SECRET_A\")\n", Some(none)),
        vec![]
    );
    assert_eq!(
        detect("a.test.ts", "const a = process.env.SECRET_A;\n", Some(none)),
        vec![]
    );
    // A write is not a read.
    assert_eq!(
        detect("a.py", "os.environ[\"SECRET_A\"] = \"1\"\n", Some(none)),
        vec![]
    );
    // One report per variable and file, at the first read.
    assert_eq!(
        detect(
            "a.ts",
            "const a = process.env.SECRET_A;\nconst b = process.env.SECRET_A;\n",
            Some(none)
        ),
        vec![(1, 11, "medium".to_string())]
    );
    assert_eq!(
        detect(
            "a.ts",
            "const a = process.env.SECRET_A;\nconst b = process.env.SECRET_B;\n",
            Some(none)
        )
        .len(),
        2
    );
    // Two variables on one line are one hit: one hit per line and rule.
    assert_eq!(
        detect(
            "a.ts",
            "const a = process.env.SECRET_A + process.env.SECRET_B;\n",
            Some(none)
        )
        .len(),
        1
    );
}

/// (line, column, confidence, severity) of every hit.
fn graded(path: &str, source: &str) -> Vec<(u32, u32, String, Option<String>)> {
    let none: &[RepoFile] = &[];
    let syntax = parse(Path::new(path), source).expect("a supported language");
    let docs = EnvDocs::build(none);
    let input = DetectorInput::new(path, source, Some(&syntax)).with_env_docs(Some(&docs));
    env::detect(&input)
        .into_iter()
        .map(|(_, h)| {
            let text = |v: Value| v.as_str().map(str::to_owned);
            (
                h.line,
                h.columns.start,
                text(serde_json::to_value(h.confidence).unwrap()).unwrap(),
                text(serde_json::to_value(h.severity).unwrap()),
            )
        })
        .collect()
}

#[test]
fn a_variable_with_a_default_is_low_and_a_required_one_is_not() {
    // (name, path, source, optional)
    let cases: &[(&str, &str, &str, bool)] = &[
        (
            "js plain",
            "a.ts",
            "const a = process.env.SECRET_A;\n",
            false,
        ),
        (
            "js ||",
            "a.ts",
            "const a = process.env.SECRET_A || 'x';\n",
            true,
        ),
        (
            "js ??",
            "a.ts",
            "const a = process.env.SECRET_A ?? 'x';\n",
            true,
        ),
        (
            "js parenthesized",
            "a.ts",
            "const a = (process.env.SECRET_A) || 'x';\n",
            true,
        ),
        (
            "js inside a call",
            "a.ts",
            "const a = Number(process.env.SECRET_A || 1);\n",
            true,
        ),
        (
            "js index ||",
            "a.ts",
            "const a = process.env['SECRET_A'] || 'x';\n",
            true,
        ),
        (
            "js pattern default",
            "a.ts",
            "const { SECRET_A = 3000 } = process.env;\n",
            true,
        ),
        (
            "js pattern",
            "a.ts",
            "const { SECRET_A } = process.env;\n",
            false,
        ),
        (
            "js &&",
            "a.ts",
            "const a = process.env.SECRET_A && 1;\n",
            false,
        ),
        (
            "js ternary",
            "a.ts",
            "const a = process.env.SECRET_A ? 1 : 2;\n",
            false,
        ),
        (
            "js read on the right",
            "a.ts",
            "const a = x || process.env.SECRET_A;\n",
            false,
        ),
        (
            "js Deno default",
            "a.ts",
            "const a = Deno.env.get('SECRET_A') ?? 'x';\n",
            true,
        ),
        (
            "python default",
            "a.py",
            "a = os.getenv('SECRET_A', 'x')\n",
            true,
        ),
        ("python none", "a.py", "a = os.getenv('SECRET_A')\n", false),
        (
            "python environ.get default",
            "a.py",
            "a = os.environ.get('SECRET_A', 'x')\n",
            true,
        ),
        (
            "python index",
            "a.py",
            "a = os.environ['SECRET_A']\n",
            false,
        ),
        (
            "php default",
            "a.php",
            "<?php $a = env('SECRET_A', 'x');\n",
            true,
        ),
        ("php none", "a.php", "<?php $a = env('SECRET_A');\n", false),
        (
            "ruby fetch default",
            "a.rb",
            "a = ENV.fetch('SECRET_A', 'x')\n",
            true,
        ),
        (
            "ruby fetch block",
            "a.rb",
            "a = ENV.fetch('SECRET_A') { 'x' }\n",
            true,
        ),
        ("ruby fetch", "a.rb", "a = ENV.fetch('SECRET_A')\n", false),
        ("ruby index", "a.rb", "a = ENV['SECRET_A']\n", false),
        (
            "elixir default",
            "a.ex",
            "a = System.get_env(\"SECRET_A\", \"x\")\n",
            true,
        ),
        (
            "elixir none",
            "a.ex",
            "a = System.get_env(\"SECRET_A\")\n",
            false,
        ),
        (
            "dart defaultValue",
            "a.dart",
            "var a = String.fromEnvironment('SECRET_A', defaultValue: 'x');\n",
            true,
        ),
        (
            "dart const defaultValue",
            "a.dart",
            "var a = const String.fromEnvironment('SECRET_A', defaultValue: 'x');\n",
            true,
        ),
        (
            "dart none",
            "a.dart",
            "var a = String.fromEnvironment('SECRET_A');\n",
            false,
        ),
        (
            "rust unwrap_or",
            "a.rs",
            "fn f() { let a = env::var(\"SECRET_A\").unwrap_or(\"x\".into()); }\n",
            true,
        ),
        (
            "rust unwrap_or_default",
            "a.rs",
            "fn f() { let a = env::var(\"SECRET_A\").unwrap_or_default(); }\n",
            true,
        ),
        (
            "rust unwrap_or_else",
            "a.rs",
            "fn f() { let a = env::var(\"SECRET_A\").unwrap_or_else(|_| x()); }\n",
            true,
        ),
        (
            "rust ok",
            "a.rs",
            "fn f() { let a = env::var(\"SECRET_A\").ok(); }\n",
            true,
        ),
        (
            "rust is_ok",
            "a.rs",
            "fn f() { let a = env::var(\"SECRET_A\").is_ok(); }\n",
            true,
        ),
        (
            "rust option_env",
            "a.rs",
            "const A: Option<&str> = option_env!(\"SECRET_A\");\n",
            true,
        ),
        (
            "rust unwrap",
            "a.rs",
            "fn f() { let a = env::var(\"SECRET_A\").unwrap(); }\n",
            false,
        ),
        (
            "rust expect",
            "a.rs",
            "fn f() { let a = env::var(\"SECRET_A\").expect(\"set\"); }\n",
            false,
        ),
        (
            "rust env!",
            "a.rs",
            "const A: &str = env!(\"SECRET_A\");\n",
            false,
        ),
        (
            "php ??",
            "a.php",
            "<?php $a = $_ENV['SECRET_A'] ?? 'x';\n",
            true,
        ),
        (
            "php ?:",
            "a.php",
            "<?php $a = getenv('SECRET_A') ?: 'x';\n",
            true,
        ),
        (
            "php env ??",
            "a.php",
            "<?php $a = env('SECRET_A') ?? 'x';\n",
            true,
        ),
        (
            "php $_ENV",
            "a.php",
            "<?php $a = $_ENV['SECRET_A'];\n",
            false,
        ),
        (
            "python or",
            "a.py",
            "a = os.environ.get('SECRET_A') or 'x'\n",
            true,
        ),
        (
            "python index or",
            "a.py",
            "a = os.environ['SECRET_A'] or 'x'\n",
            true,
        ),
        (
            "python and",
            "a.py",
            "a = os.environ.get('SECRET_A') and 'x'\n",
            false,
        ),
        ("ruby ||", "a.rb", "a = ENV['SECRET_A'] || 'x'\n", true),
        (
            "ruby fetch ||",
            "a.rb",
            "a = ENV.fetch('SECRET_A') || 'x'\n",
            true,
        ),
        (
            "csharp ??",
            "A.cs",
            "class A { void F() { var a = Environment.GetEnvironmentVariable(\"SECRET_A\") ?? \"x\"; } }\n",
            true,
        ),
        (
            "csharp",
            "A.cs",
            "class A { void F() { var a = Environment.GetEnvironmentVariable(\"SECRET_A\"); } }\n",
            false,
        ),
        (
            "swift ??",
            "a.swift",
            "let a = getenv(\"SECRET_A\") ?? \"x\"\n",
            true,
        ),
        (
            "dart ??",
            "a.dart",
            "void f() { var a = Platform.environment['SECRET_A'] ?? 'x'; }\n",
            true,
        ),
        (
            "dart index",
            "a.dart",
            "void f() { var a = Platform.environment['SECRET_A']; }\n",
            false,
        ),
        (
            "go LookupEnv",
            "a.go",
            "package m\nfunc f() { a, ok := os.LookupEnv(\"SECRET_A\") }\n",
            true,
        ),
        (
            "go Getenv",
            "a.go",
            "package m\nfunc f() { a := os.Getenv(\"SECRET_A\") }\n",
            false,
        ),
    ];
    for (name, path, source, optional) in cases {
        let got = graded(path, source);
        assert_eq!(got.len(), 1, "{name}: {got:?}");
        let (_, _, confidence, severity) = &got[0];
        if *optional {
            assert_eq!(
                (confidence.as_str(), severity.as_deref()),
                ("low", Some("low")),
                "{name}"
            );
        } else {
            assert_eq!(
                (confidence.as_str(), severity.as_deref()),
                ("medium", None),
                "{name}"
            );
        }
    }
}

#[test]
fn variables_a_platform_or_bundler_owns_are_not_reported() {
    let none: &[RepoFile] = &[];
    for source in [
        "const a = process.env.VERCEL_URL;\n",
        "const a = process.env.NETLIFY;\n",
        "const a = process.env.CF_PAGES;\n",
        "const a = process.env.VITEST;\n",
        "const a = process.env.http_proxy;\n",
        "const a = import.meta.env.MODE;\n",
        "const a = import.meta.env.BASE_URL;\n",
        "const a = import.meta.env.DEV + import.meta.env.PROD + import.meta.env.SSR;\n",
    ] {
        assert_eq!(detect("a.ts", source, Some(none)), vec![], "{source}");
    }
    assert_eq!(
        detect("a.py", "a = os.environ.get('PYTHONSTARTUP')\n", Some(none)),
        vec![]
    );
    assert_eq!(
        detect(
            "a.py",
            "a = os.environ.get('WSL_DISTRO_NAME')\n",
            Some(none)
        ),
        vec![]
    );
    assert_eq!(
        detect(
            "a.py",
            "a = os.environ.get('SOURCE_DATE_EPOCH')\n",
            Some(none)
        ),
        vec![]
    );
    // Only `import.meta.env` has those built-ins: `process.env.MODE` is the project's own.
    assert_eq!(
        detect("a.ts", "const a = process.env.MODE;\n", Some(none)).len(),
        1
    );
    assert_eq!(
        detect("a.ts", "const a = import.meta.env.VITE_MODE;\n", Some(none)).len(),
        1
    );
}

struct FixedEnv;
impl Env for FixedEnv {
    fn now(&self) -> String {
        "2026-01-01T00:00:00.000Z".into()
    }
    fn new_id(&self) -> String {
        "id".into()
    }
}

fn policy() -> ReviewPolicy {
    serde_json::from_value(json!({
        "id": "p",
        "rules": [{
            "id": "document-env-vars",
            "statement": "Every environment variable the code reads must be documented.",
            "origin": { "kind": "pack", "packId": "debuggatha/dx", "packVersion": "1.1.0" }
        }],
        "packRefs": [], "conflicts": []
    }))
    .expect("policy")
}

fn unit(file: &str, content: &str) -> SourceUnit {
    SourceUnit {
        file: file.into(),
        content: content.into(),
        changed_lines: None,
    }
}

#[test]
fn a_review_with_the_files_gives_a_finding_and_without_them_a_gap() {
    let registry = default_registry();
    let units = [unit(
        "src/app.py",
        "import os\nx = os.getenv(\"SECRET_A\")\n",
    )];

    let with = run_review_with_files(
        &policy(),
        &units,
        Some(&[file("README.md", "No variables here.\n")]),
        &registry,
        &FixedEnv,
    )
    .expect("run");
    assert_eq!(with.findings.len(), 1);
    let finding = serde_json::to_value(&with.findings[0]).unwrap();
    assert_eq!(finding["evidence"][1]["ruleId"], "document-env-vars");
    assert_eq!(finding["locations"][0]["file"], "src/app.py");
    assert_eq!(finding["locations"][0]["lines"]["start"], 2);

    // The same review, documented: nothing.
    let documented = run_review_with_files(
        &policy(),
        &units,
        Some(&[file(".env.example", "SECRET_A=\n")]),
        &registry,
        &FixedEnv,
    )
    .expect("run");
    assert_eq!(documented.findings.len(), 0);

    // The caller has no files: no finding, and coverage says why.
    let without = run_review(&policy(), &units, &registry, &FixedEnv).expect("run");
    assert_eq!(without.findings.len(), 0);
    let coverage =
        serde_json::to_value(without.coverage(&policy(), &registry, vec![], false)).unwrap();
    assert_eq!(coverage["complete"], false);
    assert_eq!(coverage["checkedRules"], json!([]));
    let gaps = coverage["ruleGaps"].as_array().unwrap();
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0]["reason"], "needs-repository-files");

    // With the files the rule is checked, with the languages it cannot read named as a gap.
    let coverage =
        serde_json::to_value(with.coverage(&policy(), &registry, vec![], false)).unwrap();
    assert_eq!(
        coverage["checkedRules"][0]["ruleIds"],
        json!(["document-env-vars"])
    );
    let gaps = coverage["ruleGaps"].as_array().unwrap();
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0]["reason"], "no-detector");
    assert!(gaps[0]["detail"].as_str().unwrap().contains("HTML"));
    assert_eq!(coverage["complete"], false);
}

#[test]
fn the_gap_reason_is_in_the_schema() {
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/../../docs/schemas/review-result.schema.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    let reasons = schema["definitions"]["RuleGap"]["properties"]["reason"]["enum"]
        .as_array()
        .unwrap();
    assert!(reasons.contains(&json!("needs-repository-files")));
    assert_eq!(
        serde_json::to_value(GapReason::NeedsRepositoryFiles).unwrap(),
        json!("needs-repository-files")
    );
}

#[test]
fn more_reads_and_look_alikes() {
    check_reads(&[
        (
            "a unicode-prefixed literal",
            "a.py",
            "a = os.getenv(u\"G\") + os.getenv(R\"H\")\n",
            &["G", "H"],
        ),
        (
            "a name starting with an underscore",
            "a.py",
            "a = os.getenv('_PRIVATE')\n",
            &["_PRIVATE"],
        ),
        (
            "a name starting with a digit",
            "a.py",
            "a = os.getenv('1A')\n",
            &[],
        ),
        (
            "Deno.cache.get is not the environment",
            "a.ts",
            "Deno.cache.get('A');\n",
            &[],
        ),
    ]);
}

#[test]
fn more_kinds_of_documentation_file() {
    use DocKind::{Config, Dotenv, Markdown};
    for (path, want) in [
        ("docs/guide.mdx", Some(Markdown)),
        ("app.env.sample", Some(Dotenv)),
        (".env.foo.example", None),
        (".circleci/config.yml", Some(Config)),
        (".cargo/config", Some(Config)),
        ("rules.mk", Some(Config)),
        (".github\\workflows\\ci.yml", Some(Config)),
    ] {
        assert_eq!(doc_kind(path), want, "{path}");
    }
}

#[test]
fn more_of_what_a_file_mentions() {
    mentions(
        "README.md",
        "Words: `Mixed_Case`, `X`, `_1`, `__`.\n\n```sh\nKEEP_A\n~~~\nKEEP_B\n```\n",
        &["KEEP_A", "KEEP_B"],
        &["Mixed_Case", "X", "_1", "__"],
    );
    mentions(
        "Dockerfile",
        "ENV QUOTED='x y=z' LAST=1\n# uses $IN_COMMENT\n",
        &["QUOTED", "LAST"],
        &["y", "IN_COMMENT"],
    );
    mentions(
        "docker-compose.yml",
        "services:\n  a:\n    environment:\n      - TRAILING # the last one\n    url: SERVER:8080\n    cond: TEST_VAR == 1\n    scope: SCOPE::inner\n",
        &["TRAILING"],
        &["SERVER", "TEST_VAR", "SCOPE"],
    );
    mentions("Makefile", "FLAGS += -O2\n", &["FLAGS"], &[]);
    let docs = EnvDocs::build(&[file("src/main.rs", "A=1\n"), file("README.md", "`X_Y`\n")]);
    assert_eq!(docs.files_read(), 1);
}

#[test]
fn the_files_of_a_repository_that_are_read() {
    let dir = std::env::temp_dir().join(format!("debuggatha-env-{}", std::process::id()));
    drop(std::fs::remove_dir_all(&dir));
    let write = |path: &str, content: &str| {
        let full = dir.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, content).unwrap();
    };
    write("README.md", "x");
    write(".env.example", "A=1");
    write(".github/workflows/ci.yml", "x");
    write("services/api/Dockerfile", "x");
    write("src/main.rs", "x");
    write("node_modules/pkg/README.md", "x");
    write("vendor/lib/README.md", "x");
    write("docs/huge.md", &"x".repeat(1_048_577));
    let listed = debuggatha_engine::scan_scope::list_repo_files(&dir);
    drop(std::fs::remove_dir_all(&dir));
    assert_eq!(
        listed,
        vec![
            ".env.example",
            ".github/workflows/ci.yml",
            "README.md",
            "services/api/Dockerfile",
        ]
    );
}

#[test]
fn a_read_in_a_region_the_parser_could_not_read_is_not_concluded_about() {
    let broken = "a = os.getenv(\"SECRET_A\" 1 + (\n";
    let syntax = parse(Path::new("a.py"), broken).expect("python");
    let input = DetectorInput::new("a.py", broken, Some(&syntax));
    assert_eq!(env::reads_of(&input), vec![]);
    // A read elsewhere in the same file is still read.
    let mixed = "a = os.getenv(\"SECRET_B\")\nb = (\n";
    let syntax = parse(Path::new("a.py"), mixed).expect("python");
    let input = DetectorInput::new("a.py", mixed, Some(&syntax));
    let names: Vec<_> = env::reads_of(&input).into_iter().map(|r| r.name).collect();
    assert_eq!(names, vec!["SECRET_B"]);
}

#[test]
fn more_optional_reads() {
    for (name, path, source) in [
        (
            "rust map_or",
            "a.rs",
            "fn f() { let a = env::var(\"SECRET_A\").map_or(false, |v| v == \"1\"); }\n",
        ),
        (
            "rust map_or_else",
            "a.rs",
            "fn f() { let a = env::var(\"SECRET_A\").map_or_else(|_| 1, |v| v.len()); }\n",
        ),
    ] {
        let got = graded(path, source);
        assert_eq!(got.len(), 1, "{name}: {got:?}");
        assert_eq!(got[0].2, "low", "{name}");
    }
}

#[test]
fn rust_macros_inside_macros_and_attributes() {
    check_reads(&[
        (
            "nested in include_str!",
            "a.rs",
            "pub const A: &str = include_str!(env!(\"BAT_GENERATED\"));\n",
            &["BAT_GENERATED"],
        ),
        (
            "in an attribute",
            "a.rs",
            "#[command(version = env!(\"CARGO_PKG_VERSION\"))]\nstruct A;\n",
            &["CARGO_PKG_VERSION"],
        ),
        (
            "nested in concat!",
            "a.rs",
            "const A: &str = concat!(env!(\"FIRST\"), \"/x\", option_env!(\"SECOND\"));\n",
            &["FIRST", "SECOND"],
        ),
        (
            "another macro in a macro",
            "a.rs",
            "const A: &str = concat!(format!(\"FIRST\"), \"/x\");\n",
            &[],
        ),
    ]);
}

#[test]
fn a_build_script_defines_the_variables_it_prints() {
    // The repository sets `NAME` itself, so `env!("NAME")` is not something to document.
    let defined =
        |path: &str, content: &str| EnvDocs::build(&[file(path, content)]).documents("BAT_GEN");
    let print = "println!(\"cargo:rustc-env=BAT_GEN={}\", x);\n";
    assert!(defined("build.rs", print));
    assert!(defined("crates/a/build.rs", print));
    assert!(defined(
        "build.rs",
        "println!(\"cargo::rustc-env=BAT_GEN=1\");\n"
    ));
    // Not a definition: another directive, a name without a value, an ordinary source file.
    assert!(!defined(
        "build.rs",
        "println!(\"cargo:rustc-cfg=BAT_GEN\");\n"
    ));
    assert!(!defined(
        "build.rs",
        "println!(\"cargo:rustc-env=BAT_GEN\");\n"
    ));
    assert!(!defined("src/main.rs", print));
    assert_eq!(doc_kind("build.rs"), Some(DocKind::RustSource));
    assert_eq!(doc_kind("src/main.rs"), None);
    // Only a file that defines something counts as read.
    assert_eq!(
        EnvDocs::build(&[file("build.rs", "fn main() {}\n")]).files_read(),
        0
    );
    assert_eq!(EnvDocs::build(&[file("build.rs", print)]).files_read(), 1);
}

#[test]
fn variables_a_platform_or_tool_owns_are_not_reported() {
    let docs = EnvDocs::build(&[]);
    let mut seen = Vec::new();
    for (path, src, name) in [
        ("a.ts", "const a = process.env.CONTEXT;\n", "CONTEXT"),
        ("a.ts", "const a = process.env.SST;\n", "SST"),
        (
            "a.ts",
            "const a = process.env.PUPPETEER_CACHE_DIR;\n",
            "PUPPETEER_CACHE_DIR",
        ),
        (
            "a.py",
            "import os\na = os.environ.get('FLASK_RUN_FROM_CLI')\n",
            "FLASK_RUN_FROM_CLI",
        ),
        (
            "a.cpp",
            "const char* a = std::getenv(\"PATHEXT\");\n",
            "PATHEXT",
        ),
        (
            "a.ex",
            "a = System.get_env(\"RELEASE_NAME\")\n",
            "RELEASE_NAME",
        ),
    ] {
        let syntax = parse(Path::new(path), src).expect("a supported language");
        let input = DetectorInput::new(path, src, Some(&syntax)).with_env_docs(Some(&docs));
        if !env::detect(&input).is_empty() {
            seen.push(name);
        }
    }
    assert!(seen.is_empty(), "reported: {seen:?}");
    // Look-alikes the project owns are still reported.
    for (path, src) in [
        ("a.ts", "const a = process.env.CONTEXT_ID;\n"),
        ("a.ts", "const a = process.env.RELEASE_TOKEN;\n"),
        ("a.ts", "const a = process.env.MYFLASK_X;\n"),
    ] {
        let syntax = parse(Path::new(path), src).expect("a supported language");
        let input = DetectorInput::new(path, src, Some(&syntax)).with_env_docs(Some(&docs));
        assert_eq!(env::detect(&input).len(), 1, "{src}");
    }
}

#[test]
fn the_scripts_of_single_file_components() {
    // Offsets are the file's: the hit lands on its own line, after markup and multibyte text.
    let vue = "<template>\n  <p>ñandú</p>\n</template>\n<script setup lang=\"ts\">\nconst a = import.meta.env.VITE_VUE;\n</script>\n";
    let svelte = "<h1>hola</h1>\n<script>\n  const a = process.env.SVELTE_X;\n</script>\n<style>\n  p { color: red; }\n</style>\n";
    let astro = "---\nconst a = import.meta.env.ASTRO_X;\nif (!a) return Astro.redirect('/');\n---\n<h1>{a}</h1>\n<script>\n  const b = process.env.ASTRO_CLIENT;\n</script>\n";
    check_reads(&[
        ("vue", "a.vue", vue, &["VITE_VUE"]),
        ("svelte", "a.svelte", svelte, &["SVELTE_X"]),
        ("astro", "a.astro", astro, &["ASTRO_X", "ASTRO_CLIENT"]),
        (
            "two scripts",
            "a.vue",
            "<script>\nconst a = process.env.ONE;\n</script>\n<script setup>\nconst b = process.env.TWO;\n</script>\n",
            &["ONE", "TWO"],
        ),
        (
            "return before the read in a frontmatter",
            "a.astro",
            "---\nif (Astro.request.method === 'POST') return new Response(null);\nconst a = import.meta.env.AFTER_RETURN;\n---\n",
            &["AFTER_RETURN"],
        ),
        (
            "jsx in a plain script",
            "a.vue",
            "<script>\nconst el = <div>{process.env.JSX_X}</div>;\n</script>\n",
            &["JSX_X"],
        ),
        // Not code: a template expression's text, a style, a script in another language, data.
        (
            "markup and style text",
            "a.vue",
            "<template><p>process.env.NOPE</p></template>\n<style>\n/* process.env.NOPE */\n</style>\n",
            &[],
        ),
        (
            "foreign language",
            "a.vue",
            "<script lang=\"coffee\">\nx = process.env.NOPE\n</script>\n",
            &[],
        ),
        (
            "json script",
            "a.svelte",
            "<script type=\"application/json\">{\"a\": \"process.env.NOPE\"}</script>\n",
            &[],
        ),
        // A string inside an Astro frontmatter that looks like a script block is still the frontmatter.
        (
            "script in a string",
            "a.astro",
            "---\nconst s = '<script>process.env.NOPE</script>';\n---\n",
            &[],
        ),
    ]);

    // Through the rule: the line and the column are the file's.
    let docs = EnvDocs::build(&[]);
    let syntax = parse(Path::new("a.vue"), vue).expect("vue");
    let input = DetectorInput::new("a.vue", vue, Some(&syntax)).with_env_docs(Some(&docs));
    let hits = env::detect(&input);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].1.line, 5);
    assert_eq!(hits[0].1.columns.start, 11);
    assert_eq!(hits[0].1.columns.end, 35);
    // A documented one is not reported.
    let docs = EnvDocs::build(&[file(".env.example", "VITE_VUE=1\n")]);
    let input = DetectorInput::new("a.vue", vue, Some(&syntax)).with_env_docs(Some(&docs));
    assert_eq!(env::detect(&input).len(), 0);
}
