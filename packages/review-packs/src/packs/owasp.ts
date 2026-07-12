import type { ReviewPack } from "@debuggatha/knowledge-system";

export const owaspPack: ReviewPack = {
  id: "debuggatha/owasp",
  version: "2.0.0",
  kind: "concern",
  displayName: "OWASP Top 10",
  dependsOn: [],
  rules: [
    {
      id: "a01-broken-access-control",
      packId: "debuggatha/owasp",
      statement:
        "Enforce authorization checks on all authenticated endpoints. Ensure a user cannot access or modify resources belonging to another user (IDOR prevention).",
      category: "security",
      appliesTo: { kind: "always" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-owasp-a01"],
      contradicts: undefined,
    },
    {
      id: "a02-cryptographic-failures",
      packId: "debuggatha/owasp",
      statement:
        "Never store passwords in plaintext or with a fast general-purpose hash (MD5, SHA-1, or bare SHA-256). Use a slow, salted, password-specific algorithm (bcrypt, scrypt, or Argon2). Encrypt sensitive data in transit (TLS) and at rest.",
      category: "security",
      appliesTo: { kind: "always" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-owasp-a02"],
      contradicts: undefined,
    },
    {
      id: "a03-injection",
      packId: "debuggatha/owasp",
      statement:
        "Never interpolate user input directly into SQL queries, NoSQL queries, OS commands, or LDAP queries. Use parameterized queries or safe ORMs.",
      category: "security",
      appliesTo: { kind: "always" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-owasp-a03"],
      contradicts: undefined,
    },
    {
      id: "a04-insecure-design",
      packId: "debuggatha/owasp",
      statement:
        "Threat-model security-critical flows (authentication, payments, access control, account recovery) before implementation, not as a retrofit. Do not rely solely on input validation to compensate for a flow that is unsafe by design — for example, a password-recovery endpoint that returns a different response depending on whether the email exists.",
      category: "architecture",
      appliesTo: { kind: "always" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-owasp-a04"],
      contradicts: undefined,
    },
    {
      id: "a05-security-misconfig",
      packId: "debuggatha/owasp",
      statement:
        "Do not expose detailed error messages or stack traces in production API responses.",
      category: "security",
      appliesTo: { kind: "always" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-owasp-a05"],
      contradicts: undefined,
    },
    {
      id: "a06-vulnerable-components",
      packId: "debuggatha/owasp",
      statement:
        "Track the exact versions of every third-party dependency and remove unused ones. Do not knowingly ship a dependency with a publicly disclosed, unpatched CVE relevant to how it is used.",
      category: "security",
      appliesTo: { kind: "always" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-owasp-a06"],
      contradicts: undefined,
    },
    {
      id: "a07-auth-failures",
      packId: "debuggatha/owasp",
      statement:
        "Enforce strong password policies, implement rate limiting on login endpoints to prevent brute-forcing, and invalidate session tokens on logout.",
      category: "security",
      appliesTo: { kind: "always" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-owasp-a07"],
      contradicts: undefined,
    },
    {
      id: "a08-software-data-integrity",
      packId: "debuggatha/owasp",
      statement:
        "Verify the integrity of software updates, CI/CD pipeline artifacts, and critical serialized data (e.g. via digital signatures or checksums) before trusting or deploying them. Do not deserialize untrusted data without an integrity check.",
      category: "security",
      appliesTo: { kind: "always" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-owasp-a08"],
      contradicts: undefined,
    },
    {
      id: "a09-logging-monitoring-failures",
      packId: "debuggatha/owasp",
      statement:
        "Log authentication attempts, access-control failures, and input-validation failures with enough context to support incident investigation. Never log sensitive data — passwords, tokens, full card numbers, PII — in plaintext.",
      category: "security",
      appliesTo: { kind: "always" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-owasp-a09"],
      contradicts: undefined,
    },
    {
      id: "a10-ssrf",
      packId: "debuggatha/owasp",
      statement:
        "Validate and sanitize any user-supplied URL or hostname before the server uses it to make an outbound request. Enforce an allowlist of permitted destinations rather than a denylist, and block requests to internal/link-local address ranges.",
      category: "security",
      appliesTo: { kind: "always" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-owasp-a10"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-owasp-a01",
      title: "Broken Access Control (IDOR)",
      body: "Insecure Direct Object Reference occurs when an application provides direct access to objects based on user-supplied input (like `GET /api/users/123`). The application must explicitly verify that the currently authenticated user owns or has roles to view object `123`.",
      externalRefs: ["https://owasp.org/Top10/A01_2021-Broken_Access_Control/"],
      limitations: [
        "Static analysis can flag the absence of an authorization check, but cannot verify the check's logic is correct (e.g. an ownership check comparing the wrong field) — a present-but-wrong check reads as compliant to a purely structural scan.",
      ],
    },
    {
      id: "know-owasp-a02",
      title: "Cryptographic Failures",
      body: "Sensitive data (credentials, financial data, health data, PII) must be protected both at rest and in transit. Weak or missing encryption, deprecated algorithms (MD5, SHA-1, DES), and hardcoded keys are the most common root causes classified under this category.",
      externalRefs: ["https://owasp.org/Top10/A02_2021-Cryptographic_Failures/"],
      limitations: [
        "Not every hash use is a password hash — MD5/SHA-1 used purely for non-security checksums (e.g. cache-busting a static asset, detecting accidental file corruption) is not a cryptographic failure; the rule applies specifically to secrets, credentials, and sensitive-data protection, not hashing in general.",
      ],
    },
    {
      id: "know-owasp-a03",
      title: "Injection",
      body: "Injection vulnerabilities occur when untrusted data is sent to an interpreter as part of a command or query. Parameterized queries enforce separation between the code and the data.",
      externalRefs: ["https://owasp.org/Top10/A03_2021-Injection/"],
      limitations: [
        "Query builders and ORMs that generate parameterized SQL under the hood are safe even though the source doesn't show a literal parameterized-query call — a naive string-concatenation scan can false-positive on safe ORM usage that happens to build a string internally before parameterizing it.",
      ],
    },
    {
      id: "know-owasp-a04",
      title: "Insecure Design",
      body: "Distinct from a Security Misconfiguration or an implementation bug: this category covers flaws in the design itself, present even with a flawless implementation. Threat modeling and secure design patterns for a given business flow must happen before code is written, since some design flaws cannot be patched without reworking the flow.",
      externalRefs: ["https://owasp.org/Top10/A04_2021-Insecure_Design/"],
      limitations: [
        "This is the least mechanically detectable OWASP category — it requires understanding business intent, not just code shape, so a heuristic scan can only catch a narrow subset of known anti-patterns (e.g. account-enumeration-via-timing or -response-shape); most insecure-design issues require human threat-modeling review to catch.",
      ],
    },
    {
      id: "know-owasp-a05",
      title: "Security Misconfiguration (Information Leakage)",
      body: "Stack traces reveal internal frameworks, database structures, and sometimes even API keys. Generic errors must be returned to clients, while detailed errors are written to secure logs.",
      externalRefs: ["https://owasp.org/Top10/A05_2021-Security_Misconfiguration/"],
      limitations: [
        "Detailed error output in a local development environment or a CLI tool run by a trusted operator is not a violation — this rule targets externally reachable production API responses specifically, not every code path that can throw a detailed error.",
      ],
    },
    {
      id: "know-owasp-a06",
      title: "Vulnerable and Outdated Components",
      body: "Using components with known vulnerabilities undermines application defenses. Every dependency's version, transitive dependencies, and CVE status should be tracked continuously, not audited once at project start.",
      externalRefs: ["https://owasp.org/Top10/A06_2021-Vulnerable_and_Outdated_Components/"],
      limitations: [
        "Detecting this requires a live CVE database lookup against the exact resolved version, which a static rule statement alone cannot perform — flagging this rule from source inspection is inherently a lower-confidence heuristic than the other categories unless paired with an actual dependency-audit tool.",
      ],
    },
    {
      id: "know-owasp-a07",
      title: "Identification and Authentication Failures",
      body: "Authentication endpoints are the primary target for attackers. Missing rate limiting enables credential stuffing attacks, while failing to invalidate JWTs or Session IDs allows session hijacking.",
      externalRefs: [
        "https://owasp.org/Top10/A07_2021-Identification_and_Authentication_Failures/",
      ],
      limitations: [
        'Stateless JWT-based auth cannot be server-side "invalidated" the same way a session cookie can without an additional revocation mechanism (denylist, short expiry + refresh rotation) — flagging the absence of a literal `invalidateSession()` call as a violation would false-positive against a correctly designed short-lived-JWT architecture.',
      ],
    },
    {
      id: "know-owasp-a08",
      title: "Software and Data Integrity Failures",
      body: "Code and infrastructure that do not verify integrity are vulnerable to malicious updates. This includes insecure CI/CD pipelines, auto-update mechanisms without signature verification, and unsafe deserialization of data from an untrusted source.",
      externalRefs: ["https://owasp.org/Top10/A08_2021-Software_and_Data_Integrity_Failures/"],
      limitations: [
        "Deserializing data the application itself produced and controls the full lifecycle of (e.g. its own signed session cookie, or an internal message queue with no untrusted producer) is not the unsafe-deserialization case this rule targets — the risk is specifically deserializing data an external or untrusted party could have tampered with.",
      ],
    },
    {
      id: "know-owasp-a09",
      title: "Security Logging and Monitoring Failures",
      body: "Insufficient logging and monitoring, coupled with missing incident response, allows attackers to maintain persistence and pivot without detection. Logs must capture enough context to reconstruct an incident, without becoming a secondary leak of sensitive data themselves.",
      externalRefs: ["https://owasp.org/Top10/A09_2021-Security_Logging_and_Monitoring_Failures/"],
      limitations: [
        'This rule pulls in two opposite failure modes at once (log too little to investigate an incident vs. log sensitive data and create a new leak) — a naive scan for "any logging call near an auth/authz path" cannot distinguish compliant logging from a logging call that violates the very rule it appears to satisfy.',
      ],
    },
    {
      id: "know-owasp-a10",
      title: "Server-Side Request Forgery (SSRF)",
      body: "SSRF occurs when a web application fetches a remote resource without validating the user-supplied URL, allowing an attacker to coerce the server into making requests to unintended destinations — including internal-only services not otherwise reachable from outside the network.",
      externalRefs: ["https://owasp.org/Top10/A10_2021-Server-Side_Request_Forgery_%28SSRF%29/"],
      limitations: [
        "Server-initiated requests to a fixed, hardcoded, or config-defined URL (not derived from user input) are not SSRF — this rule applies specifically when the destination is influenced, directly or indirectly, by untrusted input.",
      ],
    },
  ],
};
