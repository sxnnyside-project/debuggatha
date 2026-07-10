import type { ReviewPack } from "@debuggatha/knowledge-system";

export const owaspPack: ReviewPack = {
  id: "debuggatha/owasp",
  version: "1.0.0",
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
  ],
  knowledge: [
    {
      id: "know-owasp-a01",
      title: "Broken Access Control (IDOR)",
      body: "Insecure Direct Object Reference occurs when an application provides direct access to objects based on user-supplied input (like `GET /api/users/123`). The application must explicitly verify that the currently authenticated user owns or has roles to view object `123`.",
      externalRefs: ["https://owasp.org/Top10/A01_2021-Broken_Access_Control/"],
    },
    {
      id: "know-owasp-a03",
      title: "Injection",
      body: "Injection vulnerabilities occur when untrusted data is sent to an interpreter as part of a command or query. Parameterized queries enforce separation between the code and the data.",
      externalRefs: ["https://owasp.org/Top10/A03_2021-Injection/"],
    },
    {
      id: "know-owasp-a05",
      title: "Security Misconfiguration (Information Leakage)",
      body: "Stack traces reveal internal frameworks, database structures, and sometimes even API keys. Generic errors must be returned to clients, while detailed errors are written to secure logs.",
      externalRefs: ["https://owasp.org/Top10/A05_2021-Security_Misconfiguration/"],
    },
    {
      id: "know-owasp-a07",
      title: "Identification and Authentication Failures",
      body: "Authentication endpoints are the primary target for attackers. Missing rate limiting enables credential stuffing attacks, while failing to invalidate JWTs or Session IDs allows session hijacking.",
      externalRefs: [
        "https://owasp.org/Top10/A07_2021-Identification_and_Authentication_Failures/",
      ],
    },
  ],
};
