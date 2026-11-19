//! The words the analysis recognises: request sources, sinks, validators, data-layer names.

/// Names of a data layer, whole (compared in lower case).
pub(super) const LAYER_NAMES: &[&str] = &[
    "prisma",
    "db",
    "database",
    "knex",
    "sequelize",
    "sql",
    "pool",
    "connection",
    "conn",
    "trx",
    "tx",
    "transaction",
    "entitymanager",
    "em",
    "manager",
    "queryrunner",
    "repo",
    "repository",
    "orm",
    "datasource",
];

/// Endings that make a name a data layer (`userRepository`, `orderDao`).
pub(super) const LAYER_SUFFIXES: &[&str] = &["repository", "repo", "dao", "model", "db"];

/// Properties of a request that a client controls.
pub(super) const SOURCE_PROPS: &[&str] = &["body", "query", "params", "headers", "cookies"];

/// Methods of a Hono `c.req` that return client data.
pub(super) const HONO_METHODS: &[&str] = &[
    "json",
    "text",
    "query",
    "queries",
    "param",
    "header",
    "parseBody",
    "formData",
];

/// Methods whose result still holds what the receiver held.
pub(super) const PRESERVING: &[&str] = &[
    "trim",
    "trimStart",
    "trimEnd",
    "toString",
    "toLowerCase",
    "toUpperCase",
    "toLocaleLowerCase",
    "toLocaleUpperCase",
    "replace",
    "replaceAll",
    "concat",
    "slice",
    "substring",
    "substr",
    "normalize",
    "padStart",
    "padEnd",
    "join",
    "split",
    "at",
    "filter",
    "flat",
    "find",
    "pop",
    "shift",
    "reverse",
    "sort",
    "valueOf",
    "toJSON",
];

/// Of those, the ones whose arguments end up in the result as well.
pub(super) const ARGUMENTS_TOO: &[&str] = &[
    "concat",
    "replace",
    "replaceAll",
    "padStart",
    "padEnd",
    "join",
];

/// Methods that call back with each element of the receiver.
pub(super) const ELEMENT_CALLBACKS: &[&str] = &[
    "forEach",
    "map",
    "filter",
    "flatMap",
    "some",
    "every",
    "find",
    "findIndex",
];

/// Methods of a schema or validator that turn raw data into checked data.
pub(super) const VALIDATING: &[&str] = &[
    "parse",
    "safeParse",
    "parseAsync",
    "safeParseAsync",
    "validate",
    "validateSync",
    "validateAsync",
    "isValid",
    "isValidSync",
    "cast",
    "sanitize",
];

/// Receivers whose `parse`/`validate` are not a schema's.
pub(super) const NOT_A_SCHEMA: &[&str] = &[
    "JSON",
    "Number",
    "Date",
    "Math",
    "path",
    "url",
    "URL",
    "qs",
    "querystring",
    "Object",
    "Array",
    "String",
];

/// Modules of validation and sanitizing libraries.
pub(super) const VALIDATOR_MODULES: &[&str] = &[
    "express-validator",
    "zod",
    "joi",
    "@hapi/joi",
    "yup",
    "ajv",
    "celebrate",
    "class-validator",
    "class-transformer",
    "valibot",
    "superstruct",
    "@sinclair/typebox",
    "express-joi-validation",
    "zod-express-middleware",
    "validator",
];

/// Modules that clean a query object of operators (`$ne`, `$where`).
pub(super) const MONGO_SANITIZERS: &[&str] = &["express-mongo-sanitize", "mongo-sanitize"];

/// HTTP verbs of a router, besides `use` and `all`.
pub(super) const VERBS: &[&str] = &[
    "get", "post", "put", "patch", "delete", "head", "options", "all", "use",
];

/// Query methods that need a data-layer receiver (`db.query`, but not `map.get`).
pub(super) const LAYER_SQL: &[&str] = &[
    "query", "execute", "raw", "prepare", "exec", "run", "all", "get", "each", "literal", "unsafe",
];

/// Methods that take raw SQL whatever the receiver is.
pub(super) const UNSAFE_BY_NAME: &[&str] = &[
    "$queryRawUnsafe",
    "$executeRawUnsafe",
    "whereRaw",
    "orderByRaw",
    "havingRaw",
    "groupByRaw",
    "joinRaw",
    "fromRaw",
    "selectRaw",
];

/// Query-builder methods that take a condition as text.
pub(super) const CONDITIONS: &[&str] = &["where", "andWhere", "orWhere", "having", "andHaving"];

/// Database drivers and clients that are not in the layer lists above.
pub(super) const DRIVERS: &[&str] = &[
    "client", "pg", "mysql", "mysql2", "sqlite", "sqlite3", "postgres", "dbclient", "pgpool",
];

/// Mongo methods that take a filter as their first argument.
pub(super) const MONGO_METHODS: &[&str] = &[
    "find",
    "findOne",
    "findOneAndUpdate",
    "findOneAndDelete",
    "findOneAndReplace",
    "updateOne",
    "updateMany",
    "deleteOne",
    "deleteMany",
    "countDocuments",
    "aggregate",
];

/// `child_process` functions that run a command line.
pub(super) const SHELL_FUNCTIONS: &[&str] = &["exec", "execSync"];

/// `child_process` functions whose first argument is the program to run.
pub(super) const PROGRAM_FUNCTIONS: &[&str] =
    &["spawn", "spawnSync", "execFile", "execFileSync", "fork"];

pub(super) fn lower(name: &str) -> String {
    name.to_ascii_lowercase()
}

pub(super) fn is_layer(name: &str) -> bool {
    let l = lower(name);
    LAYER_NAMES.contains(&l.as_str()) || LAYER_SUFFIXES.iter().any(|s| l.ends_with(s))
}

/// `User`, `OrderItem`: a class-like name, not a constant (`API`) or a function.
pub(super) fn is_class_like(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase) && name.chars().any(char::is_lowercase)
}

/// The words of a name: `validateUserBody` is `validate`, `user`, `body`; `shell_quote` is `shell`,
/// `quote`. Read by words so that `invalidateCache` does not say it validates.
pub(super) fn words(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in name.chars() {
        let boundary = !ch.is_alphanumeric()
            || (ch.is_uppercase() && current.chars().last().is_some_and(char::is_lowercase));
        if boundary && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        if ch.is_alphanumeric() {
            current.extend(ch.to_lowercase());
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// A name that says it checks or sanitizes: `validateBody`, `withValidation`, `sanitizeId`.
pub(super) fn is_validator_name(name: &str) -> bool {
    words(name)
        .iter()
        .any(|w| w.starts_with("valid") || w.starts_with("sanitiz") || w == "celebrate")
}

pub(super) const INJECTION: &str = "a03-injection";
pub(super) const VALIDATE: &str = "validate-input";
