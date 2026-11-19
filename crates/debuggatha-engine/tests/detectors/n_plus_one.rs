//! `n-plus-one-queries` is specified by the cases below. It reads shapes and
//! names, so every hit is low confidence and reported at medium severity; the cases say which
//! shapes count and which look-alikes do not. Mutations of the detector must fail at least one.
#![allow(clippy::expect_used, clippy::unwrap_used)]
// a failed expect is a failed test
// The tests are tables of cases: splitting a table only hides what it covers.
#![allow(clippy::too_many_lines)]

use std::path::Path;

use debuggatha_engine::detect::DetectorInput;
use debuggatha_engine::detect::rules::concepts;
use debuggatha_engine::syntax::parse;

type Expected<'a> = &'a [(u32, u32)];

fn run(file: &str, source: &str) -> Vec<(u32, u32)> {
    let syntax = parse(Path::new(file), source).expect("a supported language");
    let input = DetectorInput::new(file, source, Some(&syntax));
    concepts::detect(&input)
        .into_iter()
        .filter(|(rule, _)| *rule == "n-plus-one-queries")
        .map(|(rule, h)| {
            assert_eq!(rule, "n-plus-one-queries");
            assert_eq!(
                serde_json::to_value(h.confidence).unwrap().as_str(),
                Some("low")
            );
            assert_eq!(
                serde_json::to_value(h.severity).unwrap().as_str(),
                Some("medium")
            );
            (h.line, h.columns.start)
        })
        .collect()
}

fn check(cases: &[(&str, &str, &str, Expected<'_>)]) {
    let mut bad = Vec::new();
    for (name, file, source, want) in cases {
        let got = run(file, source);
        if got != *want {
            bad.push(format!("{name}: got {got:?}, want {want:?}"));
        }
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

/// `body` inside a `for..of` over `users`, on line 2.
macro_rules! in_for_of {
    ($body:expr) => {
        concat!("for (const u of users) {\n", $body, "\n}\n")
    };
}

#[test]
fn a_query_in_a_loop_over_a_collection() {
    check(&[
        (
            "prisma in a for..of",
            "a.ts",
            in_for_of!("  await prisma.post.findMany({ where: { authorId: u.id } });"),
            &[(2, 9)],
        ),
        (
            "for..in",
            "a.ts",
            "for (const k in rows) {\n  db.query('select 1 where id = ?', [k]);\n}\n",
            &[(2, 3)],
        ),
        (
            "a for over a length",
            "a.ts",
            "for (let i = 0; i < users.length; i++) {\n  await db.user.findUnique({ where: { id: users[i].id } });\n}\n",
            &[(2, 9)],
        ),
        (
            "a for bounded by eleven is not small",
            "a.ts",
            "for (let i = 0; i < 11; i++) {\n  await db.query(i);\n}\n",
            &[(2, 9)],
        ),
        (
            "a destructured loop variable",
            "a.ts",
            "for (const { id } of rows) {\n  await pool.query('select', [id]);\n}\n",
            &[(2, 9)],
        ),
        (
            "map with an async callback",
            "a.ts",
            "await Promise.all(users.map(async (u) => db.user.findUnique({ where: { id: u.id } })));\n",
            &[(1, 42)],
        ),
        (
            "forEach with a block",
            "a.ts",
            "users.forEach(async (u) => {\n  await repo.save(u);\n});\n",
            &[(2, 9)],
        ),
        (
            "a function expression callback",
            "a.ts",
            "users.forEach(async function (u) {\n  await repo.save(u);\n});\n",
            &[(2, 9)],
        ),
        (
            "reduce and flatMap",
            "a.ts",
            "users.reduce(async (acc, u) => {\n  await db.count(u);\n}, 0);\nusers.flatMap((u) => db.find(u));\n",
            &[(2, 9), (4, 22)],
        ),
        (
            "a computed member in the receiver",
            "a.ts",
            in_for_of!("  await db['user'].findMany(u);"),
            &[(2, 9)],
        ),
        (
            "findOneOrFail",
            "a.ts",
            in_for_of!("  await repo.findOneOrFail(u);"),
            &[(2, 9)],
        ),
        (
            "a method of this",
            "a.ts",
            in_for_of!("  await this.userRepository.findOne({ id: u.id });"),
            &[(2, 9)],
        ),
        (
            "suffixes of a data layer",
            "a.ts",
            in_for_of!("  orderDao.update(u);\n  userModel.findOne(u);\n  appDb.query(u);"),
            &[(2, 3), (3, 3), (4, 3)],
        ),
        (
            "a query builder",
            "a.ts",
            in_for_of!("  await knex('posts').where({ author: u.id }).first();"),
            &[(2, 9)],
        ),
        (
            "a builder over several lines is reported where it starts",
            "a.ts",
            in_for_of!(
                "  await db\n    .select()\n    .from(posts)\n    .where(eq(posts.author, u.id));"
            ),
            &[(2, 9)],
        ),
        (
            "a model of an ORM",
            "a.ts",
            in_for_of!(
                "  await User.findById(u.id);\n  await Order.countDocuments({ user: u.id });"
            ),
            &[(2, 9), (3, 9)],
        ),
        (
            "a tagged template",
            "a.ts",
            in_for_of!("  await sql`select * from posts where author = ${u.id}`;"),
            &[(2, 9)],
        ),
        (
            "an outer loop's variable is enough",
            "a.ts",
            "for (const a of as) {\n  for (const b of bs) {\n    await db.query(a.id);\n  }\n}\n",
            &[(3, 11)],
        ),
        (
            "a variable declared in the loop body",
            "a.ts",
            in_for_of!("  const id = u.id;\n  await db.query(id);"),
            &[(3, 9)],
        ),
        (
            "writes count too",
            "a.ts",
            in_for_of!(
                "  await prisma.post.create({ data: { author: u.id } });\n  await db.user.update(u);"
            ),
            &[(2, 9), (3, 9)],
        ),
        (
            "an arrow with an expression body",
            "a.ts",
            "const posts = users.map((u) => prisma.post.findMany({ where: { id: u.id } }));\n",
            &[(1, 32)],
        ),
        (
            "one hit per line",
            "a.ts",
            in_for_of!("  db.query(u); db.query(u);"),
            &[(2, 3)],
        ),
        (
            "a js file",
            "a.js",
            in_for_of!("  await db.query(u);"),
            &[(2, 9)],
        ),
    ]);
}

#[test]
fn what_is_not_that_pattern() {
    check(&[
        (
            "while is a retry or a cursor",
            "a.ts",
            "while (more) {\n  await db.query(page);\n}\ndo {\n  await db.query(page);\n} while (more);\n",
            &[],
        ),
        (
            "a loop over an array written out",
            "a.ts",
            "for (const t of ['a', 'b']) {\n  await db.query(t);\n}\n['a', 'b'].forEach(async (t) => {\n  await db.query(t);\n});\n",
            &[],
        ),
        (
            "ten is the largest small for",
            "a.ts",
            "for (let i = 0; i < 10; i++) {\n  await db.query(i);\n}\n",
            &[],
        ),
        (
            "a call with no receiver",
            "a.ts",
            in_for_of!("  query(u);\n  findOne(u);\n  find(u);"),
            &[],
        ),
        (
            "a receiver in lower case is not a model",
            "a.ts",
            in_for_of!("  user.findOne(u);\n  order.findById(u);"),
            &[],
        ),
        (
            "a small fixed for",
            "a.ts",
            "for (let i = 0; i < 3; i++) {\n  await db.query(i);\n}\n",
            &[],
        ),
        (
            "a query that does not use the iteration",
            "a.ts",
            in_for_of!("  await db.query(config.sql);"),
            &[],
        ),
        (
            "a variable declared outside the loop",
            "a.ts",
            "const id = 1;\nfor (const u of users) {\n  await db.query(id);\n}\n",
            &[],
        ),
        (
            "a query outside a loop",
            "a.ts",
            "await db.query(id);\nconst r = await prisma.user.findMany();\n",
            &[],
        ),
        (
            "the query in the head of the loop runs once",
            "a.ts",
            "for (const u of await db.user.findMany()) {\n  log(u);\n}\n",
            &[],
        ),
        (
            "a receiver that is not a data layer",
            "a.ts",
            in_for_of!(
                "  await cache.get(u.id);\n  await api.find(u.id);\n  users.find(u.id);\n  items.query(u.id);"
            ),
            &[],
        ),
        (
            "a method that is not a query",
            "a.ts",
            in_for_of!("  await db.close(u);\n  await prisma.$disconnect(u);\n  db.on(u);"),
            &[],
        ),
        (
            "a model name with a method every class has",
            "a.ts",
            in_for_of!(
                "  Object.create(u);\n  Promise.all(u);\n  Map.groupBy(u);\n  User.find(u);"
            ),
            &[],
        ),
        (
            "a constant is not a class",
            "a.ts",
            in_for_of!("  API.findOne(u);\n  DB_URL.findById(u);"),
            &[],
        ),
        (
            "a function made in the loop is another function",
            "a.ts",
            in_for_of!(
                "  handlers.push(async () => { await db.query(u); });\n  handlers.push(async function () { await db.query(u); });\n  function later() { return db.query(u); }"
            ),
            &[],
        ),
        (
            "a tagged template that is not sql",
            "a.ts",
            in_for_of!("  String.raw`a ${u.id}`;\n  html`<p>${u.id}</p>`;"),
            &[],
        ),
        (
            "a tagged sql that does not use the iteration",
            "a.ts",
            in_for_of!("  await sql`select 1`;"),
            &[],
        ),
        (
            "a callback of something that is not an iteration",
            "a.ts",
            "users.then(async (u) => {\n  await db.query(u);\n});\n",
            &[],
        ),
        (
            "test files seed data in a loop",
            "a.test.ts",
            in_for_of!("  await db.query(u);"),
            &[],
        ),
        (
            "a spec folder",
            "tests/seed.ts",
            in_for_of!("  await db.query(u);"),
            &[],
        ),
    ]);
}
