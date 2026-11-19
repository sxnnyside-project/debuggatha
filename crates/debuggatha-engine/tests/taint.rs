//! `a03-injection` and `validate-input` are specified by the cases below. They
//! say which flows of client data into a sink count, which look-alikes do not, and how sure the
//! detector is. Mutations of the detector must fail at least one of them.
#![allow(clippy::expect_used, clippy::unwrap_used)]
// a failed expect is a failed test
// The tests are tables of cases: splitting a table only hides what it covers.
#![allow(clippy::too_many_lines)]

use std::path::Path;

use debuggatha_engine::detect::{DetectorInput, taint};
use debuggatha_engine::syntax::parse;

/// Rule (`a03` or `val`), line, column, confidence.
type Expected<'a> = &'a [(&'a str, u32, u32, &'a str)];

fn run(file: &str, source: &str) -> Vec<(String, u32, u32, String)> {
    let syntax = parse(Path::new(file), source).expect("a supported language");
    let input = DetectorInput::new(file, source, Some(&syntax));
    taint::detect(&input)
        .into_iter()
        .map(|(rule, h)| {
            let confidence = serde_json::to_value(h.confidence)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string();
            // A low-confidence hit is the one that must not fail a run.
            let severity = serde_json::to_value(h.severity).unwrap();
            assert_eq!(
                severity.as_str(),
                (confidence == "low").then_some("medium"),
                "severity of a {confidence} hit"
            );
            let short = match rule {
                "a03-injection" => "a03",
                "validate-input" => "val",
                other => panic!("unexpected rule {other}"),
            };
            (short.to_string(), h.line, h.columns.start, confidence)
        })
        .collect()
}

/// Compares the hits of one rule (`a03` or `val`), or of both when `only` is empty.
fn check_rule(only: &str, cases: &[(&str, &str, &str, Expected<'_>)]) {
    let mut bad = Vec::new();
    for (name, file, source, want) in cases {
        let got: Vec<_> = run(file, source)
            .into_iter()
            .filter(|(rule, ..)| only.is_empty() || rule == only)
            .collect();
        let want: Vec<(String, u32, u32, String)> = want
            .iter()
            .map(|(r, l, c, k)| ((*r).to_string(), *l, *c, (*k).to_string()))
            .collect();
        if got != want {
            bad.push(format!("{name}: got {got:?}, want {want:?}"));
        }
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

fn check(cases: &[(&str, &str, &str, Expected<'_>)]) {
    check_rule("", cases);
}

fn check_a03(cases: &[(&str, &str, &str, Expected<'_>)]) {
    check_rule("a03", cases);
}

fn check_val(cases: &[(&str, &str, &str, Expected<'_>)]) {
    check_rule("val", cases);
}

/// `body` as the only statement of an Express handler, on line 2.
macro_rules! handler {
    ($body:expr) => {
        concat!("app.get('/x', async (req, res) => {\n", $body, "\n});\n")
    };
}

#[test]
fn sql_built_from_client_data() {
    check_a03(&[
        (
            "template literal in pool.query",
            "a.ts",
            handler!("  pool.query(`SELECT * FROM u WHERE id = ${req.params.id}`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "concatenation",
            "a.ts",
            handler!("  db.query(\"SELECT * FROM u WHERE n = '\" + req.query.name + \"'\");"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "through a const",
            "a.ts",
            handler!(
                "  const id = req.params.id;\n  db.query(`SELECT * FROM u WHERE id = ${id}`);"
            ),
            &[("a03", 3, 3, "high")],
        ),
        (
            "through a destructuring",
            "a.ts",
            handler!(
                "  const { id } = req.params;\n  db.query(`SELECT * FROM u WHERE id = ${id}`);"
            ),
            &[("a03", 3, 3, "high")],
        ),
        (
            "through a nested destructuring",
            "a.ts",
            handler!(
                "  const { filter: { name } } = req.body;\n  db.query(`SELECT 1 WHERE n = ${name}`);"
            ),
            &[("a03", 3, 3, "high")],
        ),
        (
            "through a string method",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE n = '${req.query.name.trim()}'`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "through a built query",
            "a.ts",
            handler!(
                "  const id = req.params.id;\n  const sql = 'SELECT * FROM u WHERE id = ' + id;\n  db.query(sql);"
            ),
            &[("a03", 4, 3, "high")],
        ),
        (
            "the whole query comes from the client",
            "a.ts",
            handler!("  db.query(req.body.sql);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "a coalesced default",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE n = ${req.query.n || 'a'}`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "a conditional",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE n = ${x ? req.query.n : 'a'}`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "a member of the body",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE n = ${req.body.user.name}`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "an indexed member",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE n = ${req.body['name']}`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "an optional member",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE n = ${req.body?.name}`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "a type assertion",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE n = ${req.body.name as string}`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "a non-null assertion",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE n = ${req.body.name!}`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "String()",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE n = ${String(req.body.name)}`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "replace with the client value as the replacement",
            "a.ts",
            handler!("  db.query('SELECT 1 WHERE n = :n'.replace(':n', req.query.n));"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "concat",
            "a.ts",
            handler!("  db.query('SELECT 1 WHERE n = '.concat(req.query.n));"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "a join of client values",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE id IN (${req.body.ids.join(',')})`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "a split value",
            "a.ts",
            handler!(
                "  const parts = req.query.q.split(' ');\n  db.query(`SELECT 1 WHERE n = '${parts[0]}'`);"
            ),
            &[("a03", 3, 3, "high")],
        ),
        (
            "an object built from the client",
            "a.ts",
            handler!(
                "  const f = { name: req.query.name };\n  db.query(`SELECT 1 WHERE n = ${f.name}`);"
            ),
            &[("a03", 3, 3, "high")],
        ),
        (
            "a for..of over client data",
            "a.ts",
            handler!(
                "  for (const id of req.body.ids) {\n    db.query(`DELETE FROM t WHERE id = ${id}`);\n  }"
            ),
            &[("a03", 3, 5, "high")],
        ),
        (
            "a callback over client data",
            "a.ts",
            handler!("  req.body.ids.forEach((id) => db.query(`DELETE FROM t WHERE id = ${id}`));"),
            &[("a03", 2, 32, "high")],
        ),
        (
            "a for..in key",
            "a.ts",
            handler!("  for (const k in req.body) {\n    db.query(`UPDATE t SET ${k} = 1`);\n  }"),
            &[("a03", 3, 5, "high")],
        ),
        (
            "a header",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE ua = '${req.headers['user-agent']}'`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "a cookie",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE s = '${req.cookies.session}'`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "a request as the first parameter of a function declaration",
            "a.ts",
            "function list(req, res) {\n  db.query(`SELECT 1 WHERE n = ${req.query.n}`);\n}\n",
            &[("a03", 2, 3, "high")],
        ),
        (
            "named request instead of req",
            "a.ts",
            "function list(request, reply) {\n  db.query(`SELECT 1 WHERE n = ${request.query.n}`);\n}\n",
            &[("a03", 2, 3, "high")],
        ),
        (
            "a class method",
            "a.ts",
            "class C {\n  list(req, res) {\n    this.db.query(`SELECT 1 WHERE n = ${req.query.n}`);\n  }\n}\n",
            &[("a03", 3, 5, "high")],
        ),
    ]);
}

#[test]
fn how_data_travels() {
    check_a03(&[
        (
            "an assignment after the closure that uses it needs another pass",
            "a.ts",
            handler!(
                "  let id = '1';\n  const run = () => db.query(`SELECT 1 WHERE id = ${id}`);\n  id = req.query.id;\n  run();"
            ),
            &[("a03", 3, 21, "low")],
        ),
        (
            "an array pattern",
            "a.ts",
            handler!(
                "  const [first] = req.body.list;\n  db.query(`SELECT 1 WHERE n = ${first}`);"
            ),
            &[("a03", 3, 3, "high")],
        ),
        (
            "a rest element",
            "a.ts",
            handler!(
                "  const { a, ...rest } = req.body;\n  db.query(`SELECT 1 WHERE n = ${rest.b}`);"
            ),
            &[("a03", 3, 3, "high")],
        ),
        (
            "a default value in a pattern",
            "a.ts",
            handler!("  const { a = 'x' } = req.body;\n  db.query(`SELECT 1 WHERE n = ${a}`);"),
            &[("a03", 3, 3, "high")],
        ),
        (
            "a nested array rest",
            "a.ts",
            handler!(
                "  const [x, ...others] = req.body.list;\n  db.query(`SELECT 1 WHERE n = ${others[0]}`);"
            ),
            &[("a03", 3, 3, "high")],
        ),
        (
            "an optional call",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE n = '${req.query.name?.trim()}'`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "an optional computed member",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE n = '${req.body?.['name']}'`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "a callback written as a function expression",
            "a.ts",
            handler!(
                "  req.body.ids.forEach(function (id) { db.query(`DELETE FROM t WHERE id = ${id}`); });"
            ),
            &[("a03", 2, 40, "high")],
        ),
        (
            "an array of client values",
            "a.ts",
            handler!(
                "  const list = [req.query.a, 'b'];\n  db.query(`SELECT 1 WHERE n IN (${list})`);"
            ),
            &[("a03", 3, 3, "high")],
        ),
        (
            "a spread into an array",
            "a.ts",
            handler!(
                "  const list = [...req.body.ids];\n  db.query(`SELECT 1 WHERE n IN (${list})`);"
            ),
            &[("a03", 3, 3, "high")],
        ),
        (
            "an object spread",
            "a.ts",
            handler!("  const f = { ...req.query };\n  db.query(`SELECT 1 WHERE n = ${f.a}`);"),
            &[("a03", 3, 3, "high")],
        ),
        (
            "a sequence",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE n = ${(0, req.query.n)}`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "an assignment as a value",
            "a.ts",
            handler!("  let v;\n  db.query(`SELECT 1 WHERE n = ${v = req.query.n}`);"),
            &[("a03", 3, 3, "high")],
        ),
        (
            "JSON.parse keeps the taint",
            "a.ts",
            handler!(
                "  const o = JSON.parse(req.body.json);\n  db.query(`SELECT 1 WHERE n = ${o.n}`);"
            ),
            &[("a03", 3, 3, "high")],
        ),
        (
            "Object.values keeps the taint",
            "a.ts",
            handler!(
                "  const v = Object.values(req.body);\n  db.query(`SELECT 1 WHERE n IN (${v})`);"
            ),
            &[("a03", 3, 3, "high")],
        ),
        (
            "decodeURIComponent keeps the taint",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE n = '${decodeURIComponent(req.query.n)}'`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "req.param()",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE id = ${req.param('id')}`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "a query string parser keeps the taint",
            "a.ts",
            handler!(
                "  const q = qs.parse(req.body.raw);\n  db.query(`SELECT 1 WHERE n = ${q.n}`);"
            ),
            &[("a03", 3, 3, "high")],
        ),
        (
            "padStart takes its filler from the client",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE n = '${'x'.padStart(5, req.query.f)}'`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "toString keeps the taint",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE n = '${req.body.n.toString()}'`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "a client value as the object of Object.keys",
            "a.ts",
            handler!(
                "  const k = Object.keys(req.body);\n  db.query(`SELECT ${k.join(',')} FROM t`);"
            ),
            &[("a03", 3, 3, "high")],
        ),
        (
            "the assignment operator keeps weak",
            "a.ts",
            handler!("  let q = 'SELECT 1 WHERE n = ';\n  q += req.query.n;\n  db.query(q);"),
            &[("a03", 4, 3, "low")],
        ),
        (
            "not a taint: Array.isArray",
            "a.ts",
            handler!("  db.query(`SELECT ${Array.isArray(req.body.a)}`);"),
            &[],
        ),
        (
            "not a taint: a number written with Number.parseInt",
            "a.ts",
            handler!("  db.query(`SELECT 1 LIMIT ${Number.parseInt(req.query.n, 10)}`);"),
            &[],
        ),
        (
            "not a taint: a unary not",
            "a.ts",
            handler!("  db.query(`SELECT ${!req.query.n}`);"),
            &[],
        ),
        (
            "not a taint: the key of a computed member",
            "a.ts",
            handler!("  db.query(`SELECT ${table[req.query.k] ? 1 : 0}`);"),
            &[],
        ),
    ]);
}

#[test]
fn request_shapes_that_are_not_client_data_and_flows_that_are() {
    check_a03(&[
        (
            "a hono request method that returns no client data",
            "a.ts",
            "app.get('/', async (c) => {\n  db.query(`SELECT ${c.req.toString()}`);\n});\n",
            &[],
        ),
        (
            "ctx.req is not hono's",
            "a.ts",
            "router.get('/', async (ctx) => {\n  db.query(`SELECT ${ctx.req.query('n')}`);\n});\n",
            &[],
        ),
        (
            "res.json() is not client data",
            "a.ts",
            handler!("  const j = await res.json();\n  db.query(`SELECT ${j.n}`);"),
            &[],
        ),
        (
            "koa: destructured from ctx",
            "a.ts",
            "router.get('/', async (ctx) => {\n  const { query } = ctx;\n  db.query(`SELECT ${query.n}`);\n});\n",
            &[("a03", 3, 3, "high")],
        ),
        (
            "the else branch of a conditional",
            "a.ts",
            handler!("  db.query(`SELECT ${x ? 'a' : req.query.n}`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "Object.hasOwn returns a boolean",
            "a.ts",
            handler!("  db.query(`SELECT ${Object.hasOwn(req.query, 'a')}`);"),
            &[],
        ),
        (
            "Array.from keeps the taint",
            "a.ts",
            handler!("  const a = Array.from(req.body.ids);\n  db.query(`SELECT ${a}`);"),
            &[("a03", 3, 3, "high")],
        ),
        (
            "a method that is not an element callback",
            "a.ts",
            handler!("  req.body.p.then((x) => db.query(`SELECT ${x}`));"),
            &[],
        ),
        (
            "a destructured let that is reassigned is low",
            "a.ts",
            handler!("  let { id } = req.params;\n  id = 'x';\n  db.query(`SELECT ${id}`);"),
            &[("a03", 4, 3, "low")],
        ),
        (
            "a destructured request that is reassigned is low",
            "a.ts",
            handler!("  let { body } = req;\n  body = 'x';\n  db.query(`SELECT ${body}`);"),
            &[("a03", 4, 3, "low")],
        ),
        (
            "a collection of a driver by name",
            "a.ts",
            handler!("  this.collection.find(req.body);"),
            &[("a03", 2, 3, "low")],
        ),
        (
            "two weak links settled only by another pass",
            "a.ts",
            handler!(
                "  let c = '1';\n  let b = '1';\n  const sink = () => db.query(`SELECT ${c}`);\n  c = b;\n  b = req.query.x;\n  sink();"
            ),
            &[("a03", 4, 22, "low")],
        ),
        (
            "an option that is not shell",
            "a.ts",
            "import { spawn } from 'child_process';\napp.post('/', (req, res) => {\n  spawn('ls', [req.query.d], { detached: true });\n});\n",
            &[],
        ),
    ]);
    check_val(&[
        (
            "request and res",
            "r.ts",
            "app.post('/u', (request, res) => {\n  save(request.body);\n});\n",
            &[("val", 2, 8, "high")],
        ),
        (
            "request and response",
            "r.ts",
            "app.post('/u', (request, response) => {\n  save(request.body);\n});\n",
            &[("val", 2, 8, "high")],
        ),
        (
            "log.info",
            "r.ts",
            "app.post('/u', (req, res) => {\n  log.info(req.body);\n});\n",
            &[],
        ),
        (
            "a call that is not a route",
            "r.ts",
            "emitter.on('x', validate, (req, res) => {\n  save(req.body);\n});\n",
            &[("val", 2, 8, "medium")],
        ),
        (
            "a function expression named in the route",
            "r.ts",
            "const create = function (req, res) {\n  save(req.body);\n};\nrouter.post('/u', create);\n",
            &[("val", 2, 8, "high")],
        ),
        (
            "a validator in one handler does not cover another",
            "r.ts",
            "app.post('/a', (req, res) => {\n  schema.parse(req.body);\n});\napp.post('/b', (req, res) => {\n  save(req.body);\n});\n",
            &[("val", 5, 8, "high")],
        ),
        (
            "a sanitizer by name",
            "r.ts",
            "app.post('/u', sanitizer(), (req, res) => {\n  save(req.body);\n});\n",
            &[],
        ),
        (
            "celebrate by name",
            "r.ts",
            "app.post('/u', celebrate({}), (req, res) => {\n  save(req.body);\n});\n",
            &[],
        ),
        (
            "a function declaration inside use() is not a validator",
            "r.ts",
            "router.use(function (req, res, next) {\n  validate(options);\n  next();\n});\nrouter.post('/u', (req, res) => {\n  save(req.body);\n});\n",
            &[("val", 6, 8, "high")],
        ),
    ]);
}

#[test]
fn other_sources() {
    check_a03(&[
        (
            "koa ctx.query",
            "a.ts",
            "router.get('/', async (ctx) => {\n  db.query(`SELECT 1 WHERE n = ${ctx.query.n}`);\n});\n",
            &[("a03", 2, 3, "high")],
        ),
        (
            "koa ctx.request.body",
            "a.ts",
            "router.post('/', async (ctx) => {\n  db.query(`SELECT 1 WHERE n = ${ctx.request.body.n}`);\n});\n",
            &[("a03", 2, 3, "high")],
        ),
        (
            "koa ctx.params",
            "a.ts",
            "router.get('/:id', async (ctx) => {\n  db.query(`SELECT 1 WHERE id = ${ctx.params.id}`);\n});\n",
            &[("a03", 2, 3, "high")],
        ),
        (
            "hono c.req.query()",
            "a.ts",
            "app.get('/', async (c) => {\n  db.query(`SELECT 1 WHERE n = ${c.req.query('n')}`);\n});\n",
            &[("a03", 2, 3, "high")],
        ),
        (
            "hono c.req.param()",
            "a.ts",
            "app.get('/:id', async (c) => {\n  db.query(`SELECT 1 WHERE id = ${c.req.param('id')}`);\n});\n",
            &[("a03", 2, 3, "high")],
        ),
        (
            "hono await c.req.json()",
            "a.ts",
            "app.post('/', async (c) => {\n  const body = await c.req.json();\n  db.query(`SELECT 1 WHERE n = ${body.n}`);\n});\n",
            &[("a03", 3, 3, "high")],
        ),
        (
            "fetch-style await request.json()",
            "a.ts",
            "export async function POST(request: Request) {\n  const { n } = await request.json();\n  db.query(`SELECT 1 WHERE n = ${n}`);\n}\n",
            &[("a03", 3, 3, "high")],
        ),
        (
            "destructured from the request object",
            "a.ts",
            handler!("  const { body } = req;\n  db.query(`SELECT 1 WHERE n = ${body.n}`);"),
            &[("a03", 3, 3, "high")],
        ),
    ]);
}

#[test]
fn query_builders_and_drivers() {
    check_a03(&[
        (
            "prisma $queryRawUnsafe",
            "a.ts",
            handler!("  prisma.$queryRawUnsafe(`SELECT 1 WHERE n = '${req.query.n}'`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "prisma $executeRawUnsafe",
            "a.ts",
            handler!("  prisma.$executeRawUnsafe('DELETE FROM t WHERE id = ' + req.params.id);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "knex.raw",
            "a.ts",
            handler!("  knex.raw(`SELECT 1 WHERE n = '${req.query.n}'`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "whereRaw on any receiver",
            "a.ts",
            handler!("  qb.whereRaw(`n = '${req.query.n}'`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "orderByRaw",
            "a.ts",
            handler!("  qb.orderByRaw(req.query.sort);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "typeorm where with a template",
            "a.ts",
            handler!("  repo.createQueryBuilder('u').where(`u.name = '${req.query.n}'`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "typeorm andWhere with a concatenation",
            "a.ts",
            handler!("  qb.andWhere('u.name = ' + req.query.n);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "sequelize.query",
            "a.ts",
            handler!("  sequelize.query(`SELECT 1 WHERE n = '${req.query.n}'`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "entity manager",
            "a.ts",
            handler!("  em.execute(`SELECT 1 WHERE n = '${req.query.n}'`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "a repository-named receiver",
            "a.ts",
            handler!("  this.userRepository.query(`SELECT 1 WHERE n = '${req.query.n}'`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "a driver client",
            "a.ts",
            handler!("  client.query(`SELECT 1 WHERE n = '${req.query.n}'`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "sqlite get",
            "a.ts",
            handler!("  db.get(`SELECT 1 WHERE n = '${req.query.n}'`);"),
            &[("a03", 2, 3, "high")],
        ),
        (
            "an awaited call",
            "a.ts",
            handler!("  const rows = await db.query(`SELECT 1 WHERE n = '${req.query.n}'`);"),
            &[("a03", 2, 22, "high")],
        ),
    ]);
}

#[test]
fn sql_that_is_not_injectable() {
    check_a03(&[
        (
            "parameters",
            "a.ts",
            handler!("  db.query('SELECT * FROM u WHERE id = $1', [req.params.id]);"),
            &[],
        ),
        (
            "question marks",
            "a.ts",
            handler!("  db.query('SELECT * FROM u WHERE id = ?', [req.params.id]);"),
            &[],
        ),
        (
            "a tagged template",
            "a.ts",
            handler!("  sql`SELECT * FROM u WHERE id = ${req.params.id}`;"),
            &[],
        ),
        (
            "a tagged template as the argument",
            "a.ts",
            handler!("  prisma.$queryRaw(sql`SELECT * FROM u WHERE id = ${req.params.id}`);"),
            &[],
        ),
        (
            "a string with no interpolation",
            "a.ts",
            handler!("  db.query('SELECT * FROM u');"),
            &[],
        ),
        (
            "a template of constants",
            "a.ts",
            handler!("  db.query(`SELECT * FROM ${TABLE}`);"),
            &[],
        ),
        (
            "Number()",
            "a.ts",
            handler!(
                "  const id = Number(req.params.id);\n  db.query(`SELECT 1 WHERE id = ${id}`);"
            ),
            &[],
        ),
        (
            "parseInt",
            "a.ts",
            handler!(
                "  const id = parseInt(req.params.id, 10);\n  db.query(`SELECT 1 WHERE id = ${id}`);"
            ),
            &[],
        ),
        (
            "parseFloat",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE p > ${parseFloat(req.query.p)}`);"),
            &[],
        ),
        (
            "Math",
            "a.ts",
            handler!("  db.query(`SELECT 1 LIMIT ${Math.min(req.query.n, 50)}`);"),
            &[],
        ),
        (
            "a unary plus",
            "a.ts",
            handler!("  db.query(`SELECT 1 LIMIT ${+req.query.n}`);"),
            &[],
        ),
        (
            "a comparison",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE a = ${req.query.n === 'x'}`);"),
            &[],
        ),
        (
            "a length",
            "a.ts",
            handler!("  db.query(`SELECT 1 LIMIT ${req.body.items.length * 2}`);"),
            &[],
        ),
        (
            "a schema parse",
            "a.ts",
            handler!(
                "  const data = schema.parse(req.body);\n  db.query(`SELECT 1 WHERE n = '${data.n}'`);"
            ),
            &[],
        ),
        (
            "a schema safeParse",
            "a.ts",
            handler!(
                "  const r = schema.safeParse(req.body);\n  db.query(`SELECT 1 WHERE n = '${r.data.n}'`);"
            ),
            &[],
        ),
        (
            "a validate call",
            "a.ts",
            handler!("  const v = validate(req.body);\n  db.query(`SELECT 1 WHERE n = '${v.n}'`);"),
            &[],
        ),
        (
            "an escape",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE n = ${escapeSql(req.query.n)}`);"),
            &[],
        ),
        (
            "mysql escape",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE n = ${mysql.escape(req.query.n)}`);"),
            &[],
        ),
        (
            "a sanitizer",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE n = ${sanitizeName(req.query.n)}`);"),
            &[],
        ),
        (
            "an unknown helper is not followed",
            "a.ts",
            handler!("  const id = toId(req.params.id);\n  db.query(`SELECT 1 WHERE id = ${id}`);"),
            &[],
        ),
        (
            "a receiver that is not a data layer",
            "a.ts",
            handler!("  cache.get(`user:${req.params.id}`);"),
            &[],
        ),
        (
            "a map",
            "a.ts",
            handler!("  users.get(`${req.params.id}`);"),
            &[],
        ),
        (
            "an array map with a numeric callback",
            "a.ts",
            handler!(
                "  const ids = req.body.ids.map(Number);\n  db.query(`DELETE FROM t WHERE id IN (${ids})`);"
            ),
            &[],
        ),
        (
            "the condition does not take a plain column",
            "a.ts",
            handler!("  qb.where(req.query.filter);"),
            &[],
        ),
        (
            "knex where with a value",
            "a.ts",
            handler!("  knex('u').where('id', req.params.id);"),
            &[],
        ),
        (
            "not a request: a local named req",
            "a.ts",
            "function f() {\n  const req = { body: { id: 1 } };\n  db.query(`SELECT 1 WHERE id = ${req.body.id}`);\n}\n",
            &[],
        ),
        (
            "not a request: the second parameter",
            "a.ts",
            "function f(a, req) {\n  db.query(`SELECT 1 WHERE id = ${req.body.id}`);\n}\n",
            &[],
        ),
        (
            "not a request: no symbol",
            "a.ts",
            "db.query(`SELECT 1 WHERE id = ${req.body.id}`);\n",
            &[],
        ),
        (
            "a test file",
            "a.test.ts",
            handler!("  db.query(`SELECT 1 WHERE n = '${req.query.n}'`);"),
            &[],
        ),
        (
            "a dummy property that is not a source",
            "a.ts",
            handler!("  db.query(`SELECT 1 WHERE m = '${req.method}'`);"),
            &[],
        ),
        (
            "a file with no request",
            "a.ts",
            "db.query(`SELECT 1 WHERE n = '${name}'`);\n",
            &[],
        ),
    ]);
}

#[test]
fn how_sure_the_flow_is() {
    check_a03(&[
        (
            "a reassigned let is low",
            "a.ts",
            handler!(
                "  let id = req.params.id;\n  id = id || '1';\n  db.query(`SELECT 1 WHERE id = ${id}`);"
            ),
            &[("a03", 4, 3, "low")],
        ),
        (
            "a let assigned once is certain",
            "a.ts",
            handler!("  let id = req.params.id;\n  db.query(`SELECT 1 WHERE id = ${id}`);"),
            &[("a03", 3, 3, "high")],
        ),
        (
            "a late assignment of client data is low",
            "a.ts",
            handler!(
                "  let id = '1';\n  if (req.query.id) id = req.query.id;\n  db.query(`SELECT 1 WHERE id = ${id}`);"
            ),
            &[("a03", 4, 3, "low")],
        ),
        (
            "a var assigned twice is low",
            "a.ts",
            handler!(
                "  var id = req.params.id;\n  id = id.trim();\n  db.query(`SELECT 1 WHERE id = ${id}`);"
            ),
            &[("a03", 4, 3, "low")],
        ),
        (
            "a chain of consts stays certain",
            "a.ts",
            handler!(
                "  const a = req.params.id;\n  const b = `x${a}`;\n  const c = b.trim();\n  db.query(`SELECT 1 WHERE id = '${c}'`);"
            ),
            &[("a03", 5, 3, "high")],
        ),
        (
            "one weak link makes the chain low",
            "a.ts",
            handler!(
                "  let a = req.params.id;\n  a = a + '';\n  const b = a;\n  db.query(`SELECT 1 WHERE id = '${b}'`);"
            ),
            &[("a03", 5, 3, "low")],
        ),
        (
            "a sure and a weak flow into one sink are sure",
            "a.ts",
            handler!(
                "  let a = '1';\n  a = req.query.a;\n  db.query(`SELECT 1 WHERE n = '${a}${req.query.b}'`);"
            ),
            &[("a03", 4, 3, "high")],
        ),
    ]);
}

#[test]
fn shell_commands() {
    check_a03(&[
        (
            "exec with a template",
            "a.ts",
            "import { exec } from 'child_process';\napp.get('/', (req, res) => {\n  exec(`ls ${req.query.dir}`);\n});\n",
            &[("a03", 3, 3, "high")],
        ),
        (
            "execSync with a concatenation",
            "a.ts",
            "import { execSync } from 'node:child_process';\napp.post('/', (req, res) => {\n  execSync('git ' + req.body.args);\n});\n",
            &[("a03", 3, 3, "high")],
        ),
        (
            "namespace import",
            "a.ts",
            "import * as cp from 'child_process';\napp.post('/', (req, res) => {\n  cp.exec(req.body.cmd);\n});\n",
            &[("a03", 3, 3, "high")],
        ),
        (
            "default import",
            "a.ts",
            "import child_process from 'child_process';\napp.post('/', (req, res) => {\n  child_process.execSync(req.body.cmd);\n});\n",
            &[("a03", 3, 3, "high")],
        ),
        (
            "require of the whole module",
            "a.js",
            "const cp = require('child_process');\napp.post('/', (req, res) => {\n  cp.exec(req.body.cmd);\n});\n",
            &[("a03", 3, 3, "high")],
        ),
        (
            "require with destructuring",
            "a.js",
            "const { exec } = require('child_process');\napp.post('/', (req, res) => {\n  exec(req.body.cmd);\n});\n",
            &[("a03", 3, 3, "high")],
        ),
        (
            "a renamed import",
            "a.ts",
            "import { exec as run } from 'child_process';\napp.post('/', (req, res) => {\n  run(req.body.cmd);\n});\n",
            &[("a03", 3, 3, "high")],
        ),
        (
            "spawn of a client program",
            "a.ts",
            "import { spawn } from 'child_process';\napp.post('/', (req, res) => {\n  spawn(req.body.program, []);\n});\n",
            &[("a03", 3, 3, "high")],
        ),
        (
            "spawn through a shell",
            "a.ts",
            "import { spawn } from 'child_process';\napp.post('/', (req, res) => {\n  spawn('ls', [req.query.d], { shell: true });\n});\n",
            &[("a03", 3, 3, "high")],
        ),
        (
            "shelljs",
            "a.ts",
            "import shell from 'shelljs';\napp.post('/', (req, res) => {\n  shell.exec(`ls ${req.query.d}`);\n});\n",
            &[("a03", 3, 3, "high")],
        ),
        (
            "spawn with arguments and no shell",
            "a.ts",
            "import { spawn } from 'child_process';\napp.post('/', (req, res) => {\n  spawn('ls', [req.query.d]);\n});\n",
            &[],
        ),
        (
            "spawn with shell false",
            "a.ts",
            "import { spawn } from 'child_process';\napp.post('/', (req, res) => {\n  spawn('ls', [req.query.d], { shell: false });\n});\n",
            &[],
        ),
        (
            "execFile with a fixed program",
            "a.ts",
            "import { execFile } from 'child_process';\napp.post('/', (req, res) => {\n  execFile('ls', [req.query.d]);\n});\n",
            &[],
        ),
        (
            "exec of a constant",
            "a.ts",
            "import { exec } from 'child_process';\napp.post('/', (req, res) => {\n  exec('ls');\n});\n",
            &[],
        ),
        (
            "a numeric argument",
            "a.ts",
            "import { exec } from 'child_process';\napp.post('/', (req, res) => {\n  exec(`kill ${Number(req.body.pid)}`);\n});\n",
            &[],
        ),
        (
            "a require the file defines itself",
            "a.ts",
            "import { createRequire } from 'node:module';\nconst require = createRequire(import.meta.url);\nconst { exec } = require('child_process');\napp.post('/', (req, res) => {\n  exec(req.body.cmd);\n});\n",
            &[("a03", 5, 3, "high")],
        ),
        (
            "an exec that is not child_process",
            "a.ts",
            "import { exec } from './mine';\napp.post('/', (req, res) => {\n  exec(req.body.cmd);\n});\n",
            &[],
        ),
        (
            "a local exec",
            "a.ts",
            "function exec(x) {}\napp.post('/', (req, res) => {\n  exec(req.body.cmd);\n});\n",
            &[],
        ),
        (
            "exec of another module with the same method",
            "a.ts",
            "import regex from 'regex';\napp.post('/', (req, res) => {\n  regex.exec(req.body.cmd);\n});\n",
            &[],
        ),
    ]);
}

#[test]
fn nosql() {
    check_a03(&[
        (
            "the request object as a filter",
            "a.ts",
            handler!("  User.find(req.query);"),
            &[("a03", 2, 3, "low")],
        ),
        (
            "the body as a filter",
            "a.ts",
            handler!("  await User.findOne(req.body);"),
            &[("a03", 2, 9, "low")],
        ),
        (
            "a spread of the body",
            "a.ts",
            handler!("  User.find({ ...req.body, active: true });"),
            &[("a03", 2, 3, "low")],
        ),
        (
            "through a variable",
            "a.ts",
            handler!("  const filter = req.query;\n  User.find(filter);"),
            &[("a03", 3, 3, "low")],
        ),
        (
            "a collection of a driver",
            "a.ts",
            handler!("  db.collection('u').find(req.body);"),
            &[("a03", 2, 3, "low")],
        ),
        (
            "deleteMany",
            "a.ts",
            handler!("  User.deleteMany(req.body);"),
            &[("a03", 2, 3, "low")],
        ),
        (
            "$where as a quoted key",
            "a.ts",
            handler!("  User.find({ '$where': `this.name == '${req.query.n}'` });"),
            &[("a03", 2, 15, "high")],
        ),
        (
            "$where with client code",
            "a.ts",
            handler!("  User.find({ $where: `this.name == '${req.query.n}'` });"),
            &[("a03", 2, 15, "high")],
        ),
        (
            "a field of the body is not flagged",
            "a.ts",
            handler!("  User.findOne({ email: req.body.email });"),
            &[],
        ),
        (
            "an id is not flagged",
            "a.ts",
            handler!("  User.find(req.params.id);"),
            &[],
        ),
        (
            "a sanitizer middleware module is imported",
            "a.ts",
            "import mongoSanitize from 'express-mongo-sanitize';\napp.use(mongoSanitize());\napp.get('/', (req, res) => {\n  User.find(req.query);\n});\n",
            &[],
        ),
        (
            "a receiver that is not a model",
            "a.ts",
            handler!("  items.find(req.query);"),
            &[],
        ),
        (
            "a constant $where",
            "a.ts",
            handler!("  User.find({ $where: 'this.a == 1' });"),
            &[],
        ),
    ]);
}

#[test]
fn a_handler_that_reads_the_body_or_the_query() {
    check_val(&[
        (
            "the body",
            "r.ts",
            "app.post('/u', (req, res) => {\n  save(req.body);\n});\n",
            &[("val", 2, 8, "high")],
        ),
        (
            "the query",
            "r.ts",
            "app.get('/u', (req, res) => {\n  list(req.query);\n});\n",
            &[("val", 2, 8, "high")],
        ),
        (
            "a field of the body",
            "r.ts",
            "app.post('/u', (req, res) => {\n  const name = req.body.name;\n});\n",
            &[("val", 2, 16, "high")],
        ),
        (
            "a destructuring of the body",
            "r.ts",
            "app.post('/u', (req, res) => {\n  const { name } = req.body;\n});\n",
            &[("val", 2, 20, "high")],
        ),
        (
            "a destructuring of the request",
            "r.ts",
            "app.post('/u', (req, res) => {\n  const { body } = req;\n});\n",
            &[("val", 2, 11, "high")],
        ),
        (
            "body and query on two lines",
            "r.ts",
            "app.post('/u', (req, res) => {\n  a(req.body);\n  b(req.query);\n});\n",
            &[("val", 2, 5, "high"), ("val", 3, 5, "high")],
        ),
        (
            "the first read is the one reported",
            "r.ts",
            "app.post('/u', (req, res) => {\n  a(req.body);\n  b(req.body);\n});\n",
            &[("val", 2, 5, "high")],
        ),
        (
            "a function expression",
            "r.ts",
            "app.post('/u', function (req, res) {\n  save(req.body);\n});\n",
            &[("val", 2, 8, "high")],
        ),
        (
            "a read inside a callback belongs to the handler",
            "r.ts",
            "app.post('/u', (req, res) => {\n  items.forEach(() => save(req.body));\n});\n",
            &[("val", 2, 28, "high")],
        ),
        (
            "request and reply",
            "r.ts",
            "app.post('/u', (request, reply) => {\n  save(request.body);\n});\n",
            &[("val", 2, 8, "high")],
        ),
        (
            "wrapped in an async handler helper",
            "r.ts",
            "router.post('/u', asyncHandler(async (req, res) => {\n  save(req.body);\n}));\n",
            &[("val", 2, 8, "high")],
        ),
        (
            "a handler named in the route",
            "r.ts",
            "const create = (req, res) => {\n  save(req.body);\n};\nrouter.post('/u', create);\n",
            &[("val", 2, 8, "high")],
        ),
        (
            "a function declaration named in the route",
            "r.ts",
            "function create(req, res) {\n  save(req.body);\n}\nrouter.post('/u', auth, create);\n",
            &[("val", 2, 8, "high")],
        ),
        (
            "a route on route()",
            "r.ts",
            "router.route('/u').post((req, res) => {\n  save(req.body);\n});\n",
            &[("val", 2, 8, "high")],
        ),
        (
            "a middleware that is not a validator",
            "r.ts",
            "app.post('/u', auth, (req, res) => {\n  save(req.body);\n});\n",
            &[("val", 2, 8, "high")],
        ),
        (
            "a controller the file does not register is medium",
            "c.ts",
            "export async function create(req: Request, res: Response) {\n  save(req.body);\n}\n",
            &[("val", 2, 8, "medium")],
        ),
        (
            "a class method is medium",
            "c.ts",
            "class C {\n  create(req, res) {\n    save(req.body);\n  }\n}\n",
            &[("val", 3, 10, "medium")],
        ),
        (
            "a handler registered in another statement than its definition is high",
            "r.ts",
            "const h = (req, res) => {\n  save(req.query);\n};\napp.get('/u', h);\n",
            &[("val", 2, 8, "high")],
        ),
    ]);
}

#[test]
fn a_handler_that_validates() {
    check_val(&[
        (
            "schema.parse of the body",
            "r.ts",
            "app.post('/u', (req, res) => {\n  const d = schema.parse(req.body);\n});\n",
            &[],
        ),
        (
            "safeParse",
            "r.ts",
            "app.post('/u', (req, res) => {\n  const d = schema.safeParse(req.body);\n});\n",
            &[],
        ),
        (
            "an awaited parseAsync",
            "r.ts",
            "app.post('/u', async (req, res) => {\n  const d = await schema.parseAsync(req.body);\n});\n",
            &[],
        ),
        (
            "Joi validate",
            "r.ts",
            "app.post('/u', (req, res) => {\n  const { error } = schema.validate(req.body);\n});\n",
            &[],
        ),
        (
            "a validate function",
            "r.ts",
            "app.post('/u', (req, res) => {\n  validate(req.body);\n});\n",
            &[],
        ),
        (
            "a name that says it validates",
            "r.ts",
            "app.post('/u', (req, res) => {\n  validateUser(req.body);\n});\n",
            &[],
        ),
        (
            "the data goes through a variable first",
            "r.ts",
            "app.post('/u', (req, res) => {\n  const { name } = req.body;\n  schema.parse({ name });\n});\n",
            &[],
        ),
        (
            "validationResult of the request",
            "r.ts",
            "app.post('/u', (req, res) => {\n  const errors = validationResult(req);\n  save(req.body);\n});\n",
            &[],
        ),
        (
            "a chain run on the request",
            "r.ts",
            "import { body } from 'express-validator';\napp.post('/u', async (req, res) => {\n  await body('name').isString().run(req);\n  save(req.body);\n});\n",
            &[],
        ),
        (
            "a validator imported from a library",
            "r.ts",
            "import { z } from 'zod';\nconst s = z.object({});\napp.post('/u', (req, res) => {\n  s.parse(req.body);\n});\n",
            &[],
        ),
        (
            "plainToInstance from class-transformer",
            "r.ts",
            "import { plainToInstance } from 'class-transformer';\napp.post('/u', (req, res) => {\n  const dto = plainToInstance(Dto, req.body);\n});\n",
            &[],
        ),
        (
            "a validator in the route",
            "r.ts",
            "app.post('/u', validate(schema), (req, res) => {\n  save(req.body);\n});\n",
            &[],
        ),
        (
            "a validator from a library in the route",
            "r.ts",
            "import { celebrate } from 'celebrate';\napp.post('/u', celebrate({}), (req, res) => {\n  save(req.body);\n});\n",
            &[],
        ),
        (
            "express-validator chains in an array",
            "r.ts",
            "import { body } from 'express-validator';\nrouter.post('/u', [body('n').isString()], (req, res) => {\n  save(req.body);\n});\n",
            &[],
        ),
        (
            "a validator before a handler named in the route",
            "r.ts",
            "const create = (req, res) => {\n  save(req.body);\n};\nrouter.post('/u', validateBody, create);\n",
            &[],
        ),
        (
            "a validator wrapping the handler",
            "r.ts",
            "router.post('/u', withValidation(schema)((req, res) => {\n  save(req.body);\n}));\n",
            &[],
        ),
        (
            "a validator on router.use",
            "r.ts",
            "router.use(validateRequest);\nrouter.post('/u', (req, res) => {\n  save(req.body);\n});\n",
            &[],
        ),
        (
            "a validator on app.use with a path",
            "r.ts",
            "app.use('/api', celebrate({}));\napp.post('/api/u', (req, res) => {\n  save(req.body);\n});\n",
            &[],
        ),
        (
            "a route with the validator on route()",
            "r.ts",
            "router.route('/u').post(validate(s), (req, res) => {\n  save(req.body);\n});\n",
            &[],
        ),
        (
            "a name that only contains the letters does not validate",
            "r.ts",
            "router.use(invalidateCache);\nrouter.post('/u', (req, res) => {\n  save(req.body);\n});\n",
            &[("val", 3, 8, "high")],
        ),
        (
            "a schema routes module is not a validator",
            "r.ts",
            "router.use(schemaRoutes);\nrouter.post('/u', (req, res) => {\n  save(req.body);\n});\n",
            &[("val", 3, 8, "high")],
        ),
        (
            "what a middleware does inside is not a validator before the route",
            "r.ts",
            "router.use((req, res, next) => {\n  validate(options);\n  next();\n});\nrouter.post('/u', (req, res) => {\n  save(req.body);\n});\n",
            &[("val", 6, 8, "high")],
        ),
        (
            "a snake_case validator name",
            "r.ts",
            "router.post('/u', validate_body, (req, res) => {\n  save(req.body);\n});\n",
            &[],
        ),
        (
            "a validator after the handler does not count",
            "r.ts",
            "app.post('/u', (req, res) => {\n  save(req.body);\n}, validate(schema));\n",
            &[("val", 2, 8, "high")],
        ),
        (
            "the validator checks something else",
            "r.ts",
            "app.post('/u', (req, res) => {\n  validate(config);\n  save(req.body);\n});\n",
            &[("val", 3, 8, "high")],
        ),
        (
            "JSON.parse is not a schema",
            "r.ts",
            "app.post('/u', (req, res) => {\n  const d = JSON.parse(req.body);\n});\n",
            &[("val", 2, 24, "high")],
        ),
    ]);
}

#[test]
fn reads_that_are_not_a_use_of_the_data() {
    check_val(&[
        (
            "console.log",
            "r.ts",
            "app.post('/u', (req, res) => {\n  console.log(req.body);\n});\n",
            &[],
        ),
        (
            "a logger",
            "r.ts",
            "app.post('/u', (req, res) => {\n  logger.info('got', req.query);\n});\n",
            &[],
        ),
        (
            "params are not in the rule",
            "r.ts",
            "app.get('/u/:id', (req, res) => {\n  find(req.params.id);\n});\n",
            &[],
        ),
        (
            "headers are not in the rule",
            "r.ts",
            "app.get('/u', (req, res) => {\n  find(req.headers.host);\n});\n",
            &[],
        ),
        (
            "a first parameter that is not a request",
            "r.ts",
            "app.get('/u', (a, b) => {\n  find(a.body);\n});\n",
            &[],
        ),
        (
            "the second parameter is not read",
            "r.ts",
            "function f(a, req) {\n  find(req.body);\n}\n",
            &[],
        ),
        (
            "a body of something else",
            "r.ts",
            "app.get('/u', (req, res) => {\n  find(res.body, other.query);\n});\n",
            &[],
        ),
        (
            "a handler that reads nothing",
            "r.ts",
            "app.get('/u', (req, res) => {\n  res.send('ok');\n});\n",
            &[],
        ),
        (
            "a test file",
            "r.test.ts",
            "app.post('/u', (req, res) => {\n  save(req.body);\n});\n",
            &[],
        ),
        ("a file with no request", "r.ts", "save(body);\n", &[]),
        (
            "a Fetch request: one parameter named request",
            "r.ts",
            "export function f(request) {\n  return request.body;\n}\n",
            &[],
        ),
        (
            "a Fetch request with a second parameter that is not the response",
            "r.ts",
            "export function f(request, options) {\n  return request.body;\n}\n",
            &[],
        ),
        (
            "a presence check with !",
            "r.ts",
            "app.post('/u', (req, res) => {\n  if (!req.body) return;\n});\n",
            &[],
        ),
        (
            "a presence check with typeof",
            "r.ts",
            "app.post('/u', (req, res) => {\n  if (typeof req.body === 'undefined') return;\n});\n",
            &[],
        ),
        (
            "a presence check as the if test",
            "r.ts",
            "app.post('/u', (req, res) => {\n  if (req.query) res.end();\n});\n",
            &[],
        ),
        (
            "a presence check on the left of &&",
            "r.ts",
            "app.post('/u', (req, res) => {\n  req.body && res.end();\n});\n",
            &[],
        ),
        (
            "the use after a presence check is reported",
            "r.ts",
            "app.post('/u', (req, res) => {\n  if (!req.body) return;\n  save(req.body);\n});\n",
            &[("val", 3, 8, "high")],
        ),
        (
            "the right of || is a use",
            "r.ts",
            "app.post('/u', (req, res) => {\n  const b = x || req.body;\n});\n",
            &[("val", 2, 18, "high")],
        ),
        (
            "the left of ?? is a use",
            "r.ts",
            "app.post('/u', (req, res) => {\n  const b = req.body ?? {};\n});\n",
            &[("val", 2, 13, "high")],
        ),
        (
            "a read next to a log is still reported",
            "r.ts",
            "app.post('/u', (req, res) => {\n  console.log('x');\n  save(req.body);\n});\n",
            &[("val", 3, 8, "high")],
        ),
    ]);
}

#[test]
fn both_rules_in_one_file() {
    check(&[(
        "an unvalidated body that reaches a query",
        "r.ts",
        "app.post('/u', (req, res) => {\n  const { id } = req.body;\n  db.query(`DELETE FROM t WHERE id = ${id}`);\n});\n",
        &[("val", 2, 18, "high"), ("a03", 3, 3, "high")],
    )]);
}

#[test]
fn a_file_the_parser_cannot_read_gives_nothing() {
    check(&[(
        "a syntax error",
        "a.ts",
        "app.get('/', (req, res) => {\n  db.query(`${req.query.x}`\n",
        &[],
    )]);
}
