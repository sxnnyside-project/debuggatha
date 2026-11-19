//! A rule over a concept reports the same thing in every language that binds it, and says nothing
//! where the language has no such thing or the code is not meant to be judged.
#![allow(clippy::expect_used, clippy::unwrap_used)] // a failed expect is a failed test

use std::path::Path;

use debuggatha_engine::detect::{DetectorInput, concepts};
use debuggatha_engine::syntax::{Language, parse};

fn hits(file: &str, source: &str) -> Vec<(&'static str, u32)> {
    let syntax = parse(Path::new(file), source).expect("a supported language");
    let input = DetectorInput::new(file, source, Some(&syntax));
    concepts::detect(&input)
        .into_iter()
        .map(|(rule, h)| (rule, h.line))
        .collect()
}

#[test]
fn dynamic_eval_is_found_in_each_language_that_has_it() {
    let cases = [
        ("a.php", "<?php\neval($x);\n"),
        ("a.py", "def f(x):\n    return eval(x)\n"),
        ("a.rb", "def f(x)\n  eval(x)\nend\n"),
        (
            "a.ex",
            "defmodule A do\n  def f(x), do: Code.eval_string(x)\nend\n",
        ),
    ];
    for (file, source) in cases {
        assert_eq!(hits(file, source), vec![("no-eval", 2)], "{file}");
    }
}

#[test]
fn a_look_alike_is_not_the_concept() {
    assert_eq!(
        hits("a.php", "<?php\n$o->eval($x);\nFoo::eval($x);\n"),
        vec![]
    );
    assert_eq!(hits("a.py", "def f(o, x):\n    return o.eval(x)\n"), vec![]);
    assert_eq!(hits("a.rb", "def f(x)\n  evaluate(x)\nend\n"), vec![]);
}

#[test]
fn a_language_without_the_concept_reports_nothing() {
    assert_eq!(
        hits("a.go", "package a\nfunc f(eval func()) { eval() }\n"),
        vec![]
    );
}

#[test]
fn panic_is_found_outside_tests_and_build_scripts() {
    assert_eq!(
        hits("lib.rs", "fn f() {\n    panic!(\"x\");\n}\n"),
        vec![("no-panic", 2)]
    );
    assert_eq!(
        hits("a.go", "package a\nfunc f() {\n\tpanic(\"x\")\n}\n"),
        vec![("no-panic", 3)]
    );
    assert_eq!(hits("build.rs", "fn main() { panic!(\"x\"); }\n"), vec![]);
    assert_eq!(
        hits("lib.rs", "#[test]\nfn t() { panic!(\"x\"); }\n"),
        vec![]
    );
    assert_eq!(
        hits("a_test.go", "package a\nfunc f() { panic(\"x\") }\n"),
        vec![]
    );
}

#[test]
fn a_shell_command_built_from_a_value_is_found_where_the_language_builds_strings() {
    let cases = [
        ("a.php", "<?php\nshell_exec(\"ls $dir\");\n"),
        ("a.php", "<?php\nsystem('ls ' . $dir);\n"),
        ("a.py", "import os\nos.system(f\"ls {d}\")\n"),
        ("a.py", "import os\nos.system(\"ls \" + d)\n"),
        ("a.rb", "system(\"ls #{d}\")\n"),
        (
            "A.java",
            "class A { void f(String d) {\nRuntime.getRuntime().exec(\"ls \" + d);\n} }",
        ),
        (
            "a.kt",
            "fun f(d: String) {\nRuntime.getRuntime().exec(\"ls \" + d)\n}",
        ),
        (
            "a.ex",
            "defmodule A do\n  def f(d), do: System.shell(\"ls #{d}\")\nend\n",
        ),
    ];
    for (file, source) in cases {
        let found = hits(file, source);
        assert!(
            found
                .iter()
                .any(|(rule, _)| *rule == "no-shell-interpolation"),
            "{file}: {found:?}"
        );
    }
}

#[test]
fn a_fixed_shell_command_is_not_reported() {
    let cases = [
        ("a.php", "<?php\nshell_exec('ls -l');\n"),
        ("a.py", "import os\nos.system(\"ls -l\")\nos.system(cmd)\n"),
        ("a.rb", "system(\"ls -l\")\n"),
        ("a.py", "def f(d):\n    run(f\"ls {d}\")\n"),
        (
            "a.php",
            "<?php\nexec('ln -s ' . escapeshellarg($a) . ' ' . escapeshellarg($b));\n",
        ),
    ];
    for (file, source) in cases {
        assert_eq!(hits(file, source), vec![], "{file}");
    }
}

fn n_plus_one(file: &str, source: &str) -> Vec<(&'static str, u32)> {
    hits(file, source)
        .into_iter()
        .filter(|(rule, _)| *rule == "n-plus-one-queries")
        .collect()
}

#[test]
fn a_query_that_reads_the_loop_variable_is_found_in_each_language() {
    let cases = [
        (
            "a.py",
            "def f(ids):\n    for i in ids:\n        cursor.execute(q, i)\n",
        ),
        (
            "a.go",
            "package a\nfunc f(ids []int) {\n\tfor _, id := range ids {\n\t\tdb.QueryRow(q, id)\n\t}\n}\n",
        ),
        (
            "a.rs",
            "fn f(ids: Vec<u8>) {\n    for id in ids {\n        sqlx::query(id);\n    }\n}\n",
        ),
        (
            "a.php",
            "<?php\nfunction f($ids) {\n    foreach ($ids as $id) {\n        $pdo->query($id);\n    }\n}\n",
        ),
        (
            "A.java",
            "class A { void f(int[] ids) {\nfor (int id : ids) {\njdbcTemplate.query(q, id);\n}\n} }",
        ),
        (
            "a.kt",
            "fun f(ids: List<Int>) {\nfor (id in ids) {\njdbcTemplate.query(q, id)\n}\n}",
        ),
        (
            "a.rb",
            "def f(ids)\n  for id in ids do\n    Foo.where(id)\n  end\nend\n",
        ),
        (
            "a.cs",
            "class A { void F(int[] ids) {\nforeach (var id in ids) {\ncontext.Find(id);\n}\n} }",
        ),
        (
            "a.ex",
            "defmodule A do\n  def f(ids) do\n    for id <- ids, do: Repo.get(User, id)\n  end\nend\n",
        ),
        (
            "a.dart",
            "void f(List ids) {\nfor (var id in ids) {\ndb.query(id);\n}\n}",
        ),
    ];
    for (file, source) in cases {
        assert_eq!(n_plus_one(file, source).len(), 1, "{file}");
    }
}

#[test]
fn a_query_that_does_not_read_the_iteration_is_left_alone() {
    let cases = [
        (
            "a.py",
            "def f(ids):\n    for i in ids:\n        cursor.execute(q)\n",
        ),
        ("a.py", "def f(ids):\n    cursor.execute(q, ids)\n"),
        (
            "a.go",
            "package a\nfunc f(ids []int, one int) {\n\tfor range ids {\n\t\tdb.QueryRow(q, one)\n\t}\n}\n",
        ),
        (
            "a_test.go",
            "package a\nfunc f(ids []int) {\n\tfor _, id := range ids {\n\t\tdb.QueryRow(q, id)\n\t}\n}\n",
        ),
    ];
    for (file, source) in cases {
        assert_eq!(n_plus_one(file, source), vec![], "{file}");
    }
}

#[test]
fn a_name_declared_in_the_loop_body_counts_as_the_iteration() {
    let source =
        "def f(rows):\n    for r in rows:\n        key = r.id\n        cursor.execute(q, key)\n";
    assert_eq!(n_plus_one("a.py", source).len(), 1);
}

fn injection(file: &str, source: &str) -> Vec<(&'static str, u32)> {
    hits(file, source)
        .into_iter()
        .filter(|(rule, _)| *rule == "a03-injection")
        .collect()
}

#[test]
fn input_that_reaches_a_sink_is_found_in_each_language() {
    let cases = [
        (
            "a.py",
            "def view(request):\n    q = request.args.get('q')\n    cursor.execute('select ' + q)\n",
        ),
        (
            "a.py",
            "import os\ndef view(request):\n    os.system(request.GET['c'])\n",
        ),
        (
            "a.php",
            "<?php\nfunction f() {\n    $id = $_GET['id'];\n    $pdo->query(\"select $id\");\n}\n",
        ),
        (
            "a.go",
            "package a\nfunc h(r *http.Request) {\n\tid := r.FormValue(\"id\")\n\tdb.Query(\"select \" + id)\n}\n",
        ),
        (
            "A.java",
            "class A { void f(R request) {\nString id = request.getParameter(\"id\");\nstmt.executeQuery(\"select \" + id);\n} }",
        ),
        (
            "a.rb",
            "def f\n  id = params[:id]\n  connection.execute(\"select #{id}\")\nend\n",
        ),
        (
            "a.cs",
            "class A { void F() {\nvar id = Request.Query[\"id\"];\ncontext.Database.ExecuteSqlRaw(\"select \" + id);\n} }",
        ),
        (
            "a.ex",
            "defmodule A do\n  def f(conn, params) do\n    Repo.query(\"select \" <> params)\n  end\nend\n",
        ),
        (
            "a.rs",
            "fn f(req: R) {\n    let id = req.query_string();\n    sqlx::query(&format!(\"select {}\", id));\n}\n",
        ),
    ];
    for (file, source) in cases {
        assert_eq!(injection(file, source).len(), 1, "{file}");
    }
}

#[test]
fn a_cast_or_a_cleaner_stops_the_flow_and_so_does_a_parameter() {
    let cases = [
        (
            "a.py",
            "def view(request):\n    q = int(request.args.get('q'))\n    cursor.execute('select ' + str(q))\n",
        ),
        (
            "a.py",
            "def view(request):\n    q = request.args.get('q')\n    cursor.execute('select 1 where a = %s', (q,))\n",
        ),
        (
            "a.php",
            "<?php\nfunction f() {\n    $id = (int) $_GET['id'];\n    $pdo->query(\"select $id\");\n}\n",
        ),
        (
            "a.go",
            "package a\nfunc h(r *http.Request) {\n\tid, _ := strconv.Atoi(r.FormValue(\"id\"))\n\tdb.Query(\"select \", id)\n}\n",
        ),
        (
            "a.py",
            "def f(name):\n    cursor.execute('select ' + name)\n",
        ),
    ];
    for (file, source) in cases {
        assert_eq!(injection(file, source), vec![], "{file}");
    }
}

#[test]
fn a_name_assigned_twice_makes_the_flow_a_guess() {
    let source = "def view(request):\n    q = request.args.get('q')\n    q = 'x'\n    cursor.execute('select ' + q)\n";
    let syntax = parse(Path::new("a.py"), source).expect("python");
    let input = DetectorInput::new("a.py", source, Some(&syntax));
    let found = concepts::detect(&input);
    let (rule, hit) = found.first().expect("a hit");
    assert_eq!(*rule, "a03-injection");
    assert_eq!(
        hit.confidence,
        debuggatha_core::review_engine::Confidence::Low
    );
}

#[test]
fn a_query_in_a_callback_over_a_collection_is_in_a_loop() {
    let cases = [
        (
            "a.kt",
            "fun f(ids: List<Int>) {\nids.forEach { id ->\njdbcTemplate.query(q, id)\n}\n}",
        ),
        (
            "A.java",
            "class A { void f(java.util.List<Integer> ids) {\nids.forEach(id -> jdbcTemplate.query(q, id));\n} }",
        ),
        (
            "a.rb",
            "def f(ids)\n  ids.each do |id|\n    Foo.where(id)\n  end\nend\n",
        ),
        (
            "a.rs",
            "fn f(ids: Vec<u8>) {\n    ids.iter().for_each(|id| {\n        sqlx::query(id);\n    });\n}\n",
        ),
        (
            "a.cs",
            "class A { void F(List<int> ids) {\nids.ForEach(id => context.Find(id));\n} }",
        ),
        (
            "a.dart",
            "void f(List ids) {\nids.forEach((id) => db.query(id));\n}",
        ),
        (
            "a.ex",
            "defmodule A do\n  def f(ids) do\n    Enum.each(ids, fn id -> Repo.get(User, id) end)\n  end\nend\n",
        ),
        (
            "a.php",
            "<?php\nfunction f($ids) {\n    array_map(fn($id) => $pdo->query($id), $ids);\n}\n",
        ),
    ];
    for (file, source) in cases {
        assert_eq!(n_plus_one(file, source).len(), 1, "{file}");
    }
}

#[test]
fn a_callback_that_does_not_iterate_is_not_a_loop() {
    let cases = [
        (
            "a.kt",
            "fun f(id: Int) {\nid.let { jdbcTemplate.query(q, it) }\n}",
        ),
        (
            "a.rb",
            "def f(id)\n  id.tap do |x|\n    Foo.where(x)\n  end\nend\n",
        ),
        (
            "a.kt",
            "fun f(ids: List<Int>, one: Int) {\nids.forEach { jdbcTemplate.query(q, one) }\n}",
        ),
    ];
    for (file, source) in cases {
        assert_eq!(n_plus_one(file, source), vec![], "{file}");
    }
}

#[test]
fn a_rule_reports_the_languages_no_concept_reaches() {
    use debuggatha_engine::detect::concepts::unbound_languages;
    let languages = [Language::Php, Language::Swift, Language::TypeScript];
    assert_eq!(unbound_languages("no-weak-random", &languages), ["swift"]);
    assert_eq!(unbound_languages("no-eval", &languages), Vec::<&str>::new());
    assert_eq!(
        unbound_languages("no-unwrap-expect", &languages),
        Vec::<&str>::new()
    );
}

#[test]
fn the_rules_of_each_language_find_what_they_name() {
    let cases = [
        (
            "no-unsafe-deserialization",
            "a.py",
            "import pickle\npickle.loads(b)\n",
        ),
        (
            "no-unsafe-deserialization",
            "a.php",
            "<?php\nunserialize($x);\n",
        ),
        ("no-unsafe-deserialization", "a.rb", "Marshal.load(x)\n"),
        (
            "no-unsafe-deserialization",
            "A.java",
            "class A { void f(java.io.ObjectInputStream in) throws Exception {\n in.readObject();\n} }\n",
        ),
        ("no-weak-hash", "a.py", "import hashlib\nhashlib.md5(b)\n"),
        (
            "no-weak-hash",
            "a.go",
            "package a\nfunc f() { md5.New() }\n",
        ),
        (
            "no-print-stack-trace",
            "A.java",
            "class A { void f(Exception e) {\n e.printStackTrace();\n} }\n",
        ),
        (
            "no-fatal-error",
            "a.swift",
            "func f() {\n    fatalError(\"x\")\n}\n",
        ),
        ("no-raw-markup", "a.rb", "def f(x)\n  raw(x)\nend\n"),
        ("no-permit-all", "a.rb", "def f\n  params.permit!\nend\n"),
        (
            "no-unbounded-c-string",
            "a.cpp",
            "void f(char* d, char* s) {\n    strcpy(d, s);\n}\n",
        ),
    ];
    for (rule, file, source) in cases {
        let found = hits(file, source);
        assert!(
            found.iter().any(|(r, _)| *r == rule),
            "{rule} in {file}: {found:?}"
        );
    }
}

#[test]
fn an_atom_is_reported_only_when_input_reaches_it() {
    let tainted =
        "defmodule A do\n  def f(conn) do\n    String.to_atom(conn.params[\"k\"])\n  end\nend\n";
    let fixed = "defmodule A do\n  def f do\n    String.to_atom(\"k\")\n  end\nend\n";
    assert_eq!(hits("a.ex", tainted), vec![("no-atom-from-input", 3)]);
    assert_eq!(hits("a.ex", fixed), vec![]);
}

#[test]
fn the_scripts_of_a_component_are_read_at_their_own_lines() {
    let vue = "<template><p>{{ x }}</p></template>\n<script setup lang=\"ts\">\nimport { db } from './db';\nfor (const id of ids) {\n  await db.user.findUnique({ where: { id } });\n}\n</script>\n";
    assert_eq!(hits("A.vue", vue), vec![("n-plus-one-queries", 5)]);
    let svelte = "<script>\nconst x = eval(code);\n</script>\n<p>{x}</p>\n";
    assert_eq!(hits("A.svelte", svelte), vec![]);
    let astro =
        "---\nconst rows = ids.map((id) => db.user.findUnique({ where: { id } }));\n---\n<p/>\n";
    assert_eq!(hits("A.astro", astro).len(), 1);
}

#[test]
fn an_empty_handler_is_reported_and_one_with_a_body_or_a_comment_is_not() {
    let cases = [
        (
            "A.java",
            "class A { void f() { try { x(); } catch (Exception e) { } } }\n",
            true,
        ),
        (
            "A.java",
            "class A { void f() { try { x(); } catch (Exception e) { log(e); } } }\n",
            false,
        ),
        (
            "A.java",
            "class A { void f() { try { x(); } catch (Exception e) { // ignored\n } } }\n",
            false,
        ),
        (
            "a.kt",
            "fun f() { try { x() } catch (e: Exception) { } }\n",
            true,
        ),
        (
            "a.kt",
            "fun f() { try { x() } catch (e: Exception) { log(e) } }\n",
            false,
        ),
        (
            "A.cs",
            "class A { void F() { try { X(); } catch (Exception e) { } } }\n",
            true,
        ),
        (
            "A.cs",
            "class A { void F() { try { X(); } catch (Exception e) { Log(e); } } }\n",
            false,
        ),
        (
            "a.php",
            "<?php\ntry { x(); } catch (Exception $e) { }\n",
            true,
        ),
        (
            "a.php",
            "<?php\ntry { x(); } catch (Exception $e) { log($e); }\n",
            false,
        ),
        ("a.py", "try:\n    x()\nexcept Exception:\n    pass\n", true),
        (
            "a.py",
            "try:\n    x()\nexcept Exception:\n    log()\n",
            false,
        ),
        ("a.rb", "begin\n  x\nrescue => e\nend\n", true),
        ("a.rb", "begin\n  x\nrescue => e\n  log(e)\nend\n", false),
        ("a.swift", "func f() { do { try x() } catch { } }\n", true),
        (
            "a.swift",
            "func f() { do { try x() } catch { log() } }\n",
            false,
        ),
        ("a.cpp", "void f() { try { x(); } catch (...) { } }\n", true),
        (
            "a.cpp",
            "void f() { try { x(); } catch (...) { log(); } }\n",
            false,
        ),
        ("a.dart", "void f() { try { x(); } catch (e) { } }\n", true),
        (
            "a.dart",
            "void f() { try { x(); } catch (e) { log(e); } }\n",
            false,
        ),
        (
            "a.ts",
            "function f() { try { x(); } catch (e) { } }\n",
            true,
        ),
        (
            "a.ts",
            "function f() { try { x(); } catch (e) { log(e); } }\n",
            false,
        ),
    ];
    for (file, source, reported) in cases {
        let found = hits(file, source)
            .iter()
            .any(|(rule, _)| *rule == "no-swallowed-error");
        assert_eq!(found, reported, "{file}: {source}");
    }
}

#[test]
fn single_language_patterns_compile_and_find_their_shape() {
    use debuggatha_engine::detect::patterns;
    assert_eq!(patterns::compile_errors(), Vec::<String>::new());
    let cases = [
        (
            "a.kt",
            "fun f(a: String?) { val b = a!!\n}\n",
            "no-double-bang",
        ),
        (
            "a.php",
            "<?php\nfunction f() { global $db; }\n",
            "no-global-variables",
        ),
        (
            "a.php",
            "<?php\nfunction f() { return $GLOBALS['a']; }\n",
            "no-global-variables",
        ),
        ("a.dart", "void f(dynamic a) {}\n", "avoid-dynamic"),
        (
            "a.dart",
            "void f(List l) { if (l.length == 0) {} }\n",
            "prefer-is-empty",
        ),
    ];
    for (file, source, rule) in cases {
        let syntax = parse(Path::new(file), source).expect("a supported language");
        let input = DetectorInput::new(file, source, Some(&syntax));
        let found: Vec<_> = patterns::detect(&input)
            .into_iter()
            .map(|(r, _)| r)
            .collect();
        assert_eq!(found, vec![rule], "{file}: {source}");
    }
    let clean = "fun f(a: String?) { val b = a ?: \"\"\n}\n";
    let syntax = parse(Path::new("a.kt"), clean).expect("kotlin");
    assert_eq!(
        patterns::detect(&DetectorInput::new("a.kt", clean, Some(&syntax))).len(),
        0
    );
}

#[test]
fn swift_and_cpp_patterns_find_their_shape() {
    use debuggatha_engine::detect::patterns;
    assert_eq!(patterns::compile_errors(), Vec::<String>::new());
    let cases = [
        ("a.swift", "func f() { let x = try! g() }\n", "no-force-try"),
        (
            "a.swift",
            "func f(b: Any) { let y = b as! String }\n",
            "no-force-cast",
        ),
        (
            "a.swift",
            "func f(a: Int?) { let z = a! }\n",
            "no-force-unwrap",
        ),
        (
            "a.cpp",
            "void f() { int* p = new int(1); }\n",
            "no-raw-new-delete",
        ),
        (
            "a.cpp",
            "void f(int* p) { delete p; }\n",
            "no-raw-new-delete",
        ),
        (
            "a.cpp",
            "void f() { char* m = malloc(3); }\n",
            "no-malloc-free",
        ),
        (
            "a.hpp",
            "using namespace std;\n",
            "no-using-namespace-in-header",
        ),
    ];
    for (file, source, rule) in cases {
        let syntax = parse(Path::new(file), source).expect("a supported language");
        let input = DetectorInput::new(file, source, Some(&syntax));
        let found: Vec<_> = patterns::detect(&input)
            .into_iter()
            .map(|(r, _)| r)
            .collect();
        assert_eq!(found, vec![rule], "{file}: {source}");
    }
    let source = "using namespace std;\n";
    let syntax = parse(Path::new("a.cpp"), source).expect("cpp");
    assert_eq!(
        patterns::detect(&DetectorInput::new("a.cpp", source, Some(&syntax))).len(),
        0
    );
    let safe = "func f(a: Int?) { let z = a ?? 0 }\n";
    let syntax = parse(Path::new("a.swift"), safe).expect("swift");
    assert_eq!(
        patterns::detect(&DetectorInput::new("a.swift", safe, Some(&syntax))).len(),
        0
    );
}

#[test]
fn patterns_of_every_language_compile() {
    use debuggatha_engine::detect::patterns;
    assert_eq!(patterns::compile_errors(), Vec::<String>::new());
}

fn rules_found(file: &str, source: &str) -> Vec<&'static str> {
    use debuggatha_engine::detect::patterns;
    let syntax = parse(Path::new(file), source).expect("a supported language");
    let input = DetectorInput::new(file, source, Some(&syntax));
    let mut rules: Vec<_> = concepts::detect(&input)
        .into_iter()
        .chain(patterns::detect(&input))
        .map(|(rule, _)| rule)
        .collect();
    rules.sort_unstable();
    rules.dedup();
    rules
}

/// A file, a source and the rule that must be found in it.
const NEW_RULE_CASES: [(&str, &str, &str); 50] = [
    (
        "a.py",
        "def f(a=[]):\n    assert a\n",
        "no-mutable-default-argument",
    ),
    (
        "a.py",
        "def f(a):\n    assert a\n",
        "no-assert-in-production",
    ),
    (
        "a.py",
        "try:\n    x()\nexcept:\n    log()\n",
        "no-bare-except",
    ),
    ("a.py", "from m import *\n", "no-wildcard-import"),
    (
        "a.py",
        "def f(u):\n    requests.get(u, verify=False)\n",
        "no-insecure-tls",
    ),
    ("a.py", "app.run(debug=True)\n", "no-flask-debug"),
    (
        "a.py",
        "try:\n    x()\nexcept Exception:\n    log()\n",
        "no-catch-all",
    ),
    ("a.py", "os.chmod(p, 0o777)\n", "no-permissive-file-mode"),
    (
        "a.py",
        "def f(u):\n    return requests.get(u)\n",
        "no-http-without-timeout",
    ),
    (
        "a.py",
        "def f(c):\n    return redirect(request.args['next'])\n",
        "no-open-redirect",
    ),
    (
        "a.py",
        "def f():\n    return render_template_string(request.args['t'])\n",
        "no-template-injection",
    ),
    (
        "a.py",
        "def f():\n    return getattr(m, request.args['n'])\n",
        "no-unsafe-reflection",
    ),
    (
        "a.py",
        "def f():\n    return re.compile(request.args['p'])\n",
        "no-regex-from-input",
    ),
    (
        "a.py",
        "import xml.etree.ElementTree as ET\ndef f(s):\n    return ET.fromstring(s)\n",
        "no-unsafe-xml-parser",
    ),
    ("a.py", "def f():\n    DES.new(k)\n", "no-weak-cipher"),
    ("a.py", "def f():\n    sys.exit(1)\n", "no-process-exit"),
    (
        "A.java",
        "import java.util.*;\nclass A {}\n",
        "no-wildcard-import",
    ),
    (
        "A.java",
        "class A { protected void finalize() {} }\n",
        "no-finalize",
    ),
    (
        "A.java",
        "class A { void f() { try { x(); } catch (Exception e) { log(e); } } }\n",
        "no-catch-all",
    ),
    (
        "A.java",
        "class A { void f() { System.exit(1); } }\n",
        "no-process-exit",
    ),
    ("A.cs", "class A { async void F() { } }\n", "no-async-void"),
    (
        "A.cs",
        "class A { void F() { lock (this) { } } }\n",
        "no-lock-this",
    ),
    (
        "A.cs",
        "class A { void F() { try { X(); } catch (Exception e) { Log(e); } } }\n",
        "no-catch-all",
    ),
    (
        "A.cs",
        "class A { void F() { GC.Collect(); } }\n",
        "no-gc-collect",
    ),
    (
        "a.go",
        "package a\nimport \"unsafe\"\n",
        "no-unsafe-package",
    ),
    (
        "a.go",
        "package a\nimport _ \"net/http/pprof\"\n",
        "no-pprof-import",
    ),
    (
        "a.go",
        "package a\nfunc f() { for i := 0; i < 3; i++ { defer g() } }\n",
        "no-defer-in-loop",
    ),
    (
        "a.go",
        "package a\nfunc f() { c := &tls.Config{InsecureSkipVerify: true}; _ = c }\n",
        "no-insecure-tls",
    ),
    (
        "a.go",
        "package a\nfunc f() { os.MkdirAll(p, 0777) }\n",
        "no-permissive-file-mode",
    ),
    (
        "a.go",
        "package a\nfunc f() { os.Exit(1) }\n",
        "no-process-exit",
    ),
    ("a.rs", "static mut X: i32 = 0;\n", "no-static-mut"),
    ("a.rs", "fn f() { todo!(); }\n", "no-unfinished-code"),
    (
        "a.rs",
        "fn f() { std::process::exit(1); }\n",
        "no-process-exit",
    ),
    (
        "a.php",
        "<?php\n$a = @file_get_contents($f);\n",
        "no-error-suppression",
    ),
    ("a.php", "<?php\nextract($_POST);\n", "no-extract"),
    ("a.php", "<?php\nmysql_query($q);\n", "no-removed-mysql-api"),
    (
        "a.php",
        "<?php\nchmod($f, 0777);\n",
        "no-permissive-file-mode",
    ),
    (
        "a.rb",
        "http.verify_mode = OpenSSL::SSL::VERIFY_NONE\n",
        "no-insecure-tls",
    ),
    (
        "a.rb",
        "begin\n  x\nrescue Exception\n  y\nend\n",
        "no-catch-all",
    ),
    (
        "a.rb",
        "class C\n  skip_forgery_protection\nend\n",
        "no-csrf-skip",
    ),
    (
        "a.kt",
        "fun f() { try { x() } catch (e: Exception) { log(e) } }\n",
        "no-catch-all",
    ),
    (
        "a.kt",
        "fun f() { GlobalScope.launch { } }\n",
        "no-global-scope",
    ),
    ("a.kt", "fun f() { runBlocking { } }\n", "no-run-blocking"),
    (
        "a.swift",
        "func f() { DispatchQueue.main.sync { } }\n",
        "no-dispatch-main-sync",
    ),
    (
        "a.swift",
        "func f(a: Int) { let b = unsafeBitCast(a, to: UInt.self) }\n",
        "no-type-punning",
    ),
    (
        "a.cpp",
        "void f(char* s) { printf(getenv(\"X\")); }\n",
        "no-format-string",
    ),
    (
        "a.cpp",
        "void f(int b) { int a = (int)b; }\n",
        "no-c-style-cast",
    ),
    (
        "a.cpp",
        "void f() { try { x(); } catch (...) { log(); } }\n",
        "no-catch-all",
    ),
    (
        "a.cpp",
        "void f(int* p) { auto q = reinterpret_cast<char*>(p); }\n",
        "no-type-punning",
    ),
    ("a.cpp", "void f() { abort(); }\n", "no-process-exit"),
];

#[test]
fn each_new_rule_finds_the_construct_it_names() {
    let mut failures = Vec::new();
    for (file, source, rule) in NEW_RULE_CASES {
        if !rules_found(file, source).contains(&rule) {
            failures.push(format!(
                "{rule} in {file}: {source:?} -> {:?}",
                rules_found(file, source)
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn rust_environment_and_leak_calls_are_reported() {
    for (source, rule) in [
        (
            "fn f() { std::env::set_var(\"A\", \"b\"); }\n",
            "no-env-mutation",
        ),
        (
            "fn f(v: Vec<u8>) { std::mem::forget(v); }\n",
            "no-leak-memory",
        ),
    ] {
        assert!(rules_found("a.rs", source).contains(&rule), "{source}");
    }
}
