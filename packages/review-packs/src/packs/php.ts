import type { ReviewPack } from "@debuggatha/knowledge-system";

export const phpPack: ReviewPack = {
  id: "debuggatha/php",
  version: "1.0.0",
  kind: "stack",
  displayName: "PHP",
  dependsOn: [],
  rules: [
    {
      id: "strict-types",
      packId: "debuggatha/php",
      statement:
        "Always declare `declare(strict_types=1);` as the first statement in every PHP file.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "php" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-php-strict-types"],
      contradicts: undefined,
    },
    {
      id: "no-eval",
      packId: "debuggatha/php",
      statement: "Never use `eval()`. It is highly dangerous and allows arbitrary code execution.",
      category: "security",
      appliesTo: { kind: "requires-language", language: "php" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-php-eval"],
      contradicts: undefined,
    },
    {
      id: "prepared-statements",
      packId: "debuggatha/php",
      statement:
        "Always use prepared statements (e.g., via PDO or mysqli) when querying databases. Never concatenate strings into SQL queries.",
      category: "security",
      appliesTo: { kind: "requires-language", language: "php" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-php-sql-injection"],
      contradicts: undefined,
    },
    {
      id: "use-password-hash",
      packId: "debuggatha/php",
      statement:
        "Use `password_hash()` and `password_verify()` for handling passwords. Never use `md5()`, `sha1()`, or custom hashing schemes.",
      category: "security",
      appliesTo: { kind: "requires-language", language: "php" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-php-passwords"],
      contradicts: undefined,
    },
    {
      id: "no-global-variables",
      packId: "debuggatha/php",
      statement:
        "Avoid using the `global` keyword or reading from `$GLOBALS`. Inject dependencies explicitly.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "php" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-php-globals"],
      contradicts: undefined,
    },
    {
      id: "type-hints",
      packId: "debuggatha/php",
      statement:
        "Provide explicit type hints for all function arguments, return types, and class properties.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "php" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-php-type-hints"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-php-strict-types",
      title: "Strict Type Enforcement",
      body: 'By default, PHP coerces types (e.g., passing a string `"1"` to an int parameter). `declare(strict_types=1);` forces a `TypeError` on mismatch, catching silent truncation or conversion bugs early.',
      externalRefs: [
        "https://www.php.net/manual/en/language.types.declarations.php#language.types.declarations.strict",
      ],
    },
    {
      id: "know-php-eval",
      title: "Eval is Evil",
      body: "`eval()` executes an arbitrary string as PHP code. If any part of that string is influenced by user input, it results in complete Remote Code Execution (RCE) on the server.",
      externalRefs: ["https://www.php.net/manual/en/function.eval.php"],
    },
    {
      id: "know-php-sql-injection",
      title: "SQL Injection Prevention",
      body: "Concatenating strings into SQL queries allows attackers to break out of the string boundary and execute arbitrary SQL commands. PDO prepared statements send the query and the data separately, neutralizing the threat.",
      externalRefs: ["https://www.php.net/manual/en/pdo.prepared-statements.php"],
    },
    {
      id: "know-php-passwords",
      title: "Cryptographic Password Hashing",
      body: "`password_hash()` utilizes strong, natively supported hashing algorithms (like bcrypt or Argon2) with built-in salting and stretching, making it resilient against rainbow tables and brute force attacks.",
      externalRefs: ["https://www.php.net/manual/en/function.password-hash.php"],
    },
    {
      id: "know-php-globals",
      title: "Global State Entanglement",
      body: "Using `global` creates invisible dependencies across the application. It breaks encapsulation, makes unit testing extremely difficult, and leads to unpredictable mutations.",
      externalRefs: ["https://phptherightway.com/#dependency_injection"],
    },
    {
      id: "know-php-type-hints",
      title: "Explicit Typings",
      body: "Using property, argument, and return type declarations creates a self-documenting API contract that the PHP runtime enforces, drastically reducing `null` checks and unexpected object shapes.",
      externalRefs: ["https://www.php.net/manual/en/language.types.declarations.php"],
    },
  ],
};
