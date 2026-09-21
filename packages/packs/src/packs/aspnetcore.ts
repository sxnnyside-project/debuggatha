import type { ReviewPack } from "@debuggatha/core";

export const aspnetcorePack: ReviewPack = {
  id: "debuggatha/aspnetcore",
  version: "1.0.0",
  kind: "stack",
  displayName: "ASP.NET Core",
  dependsOn: [],
  rules: [
    {
      id: "async-all-the-way",
      packId: "debuggatha/aspnetcore",
      statement:
        "Never call `.Wait()` or `.Result` on a Task. Always use `await` all the way up the call stack to prevent thread pool starvation.",
      category: "performance",
      appliesTo: { kind: "requires-framework", framework: "aspnetcore" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-aspnetcore-async"],
      contradicts: undefined,
    },
    {
      id: "dependency-injection-lifetimes",
      packId: "debuggatha/aspnetcore",
      statement:
        "Do not inject Scoped services into Singleton services. This causes a captive dependency, effectively turning the Scoped service into a Singleton.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "aspnetcore" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-aspnetcore-di"],
      contradicts: undefined,
    },
    {
      id: "use-ienumerable-safely",
      packId: "debuggatha/aspnetcore",
      statement:
        "Avoid multiple enumerations of `IEnumerable<T>`. Call `.ToList()` or `.ToArray()` if the data must be iterated over multiple times.",
      category: "performance",
      appliesTo: { kind: "requires-framework", framework: "aspnetcore" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-aspnetcore-ienumerable"],
      contradicts: undefined,
    },
    {
      id: "anti-forgery-tokens",
      packId: "debuggatha/aspnetcore",
      statement:
        "Always apply `[ValidateAntiForgeryToken]` to state-mutating actions (POST, PUT, DELETE) in MVC or Razor Pages.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "aspnetcore" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-aspnetcore-csrf"],
      contradicts: undefined,
    },
    {
      id: "entity-framework-tracking",
      packId: "debuggatha/aspnetcore",
      statement:
        "Use `.AsNoTracking()` on Entity Framework queries when the entities are strictly read-only and will not be updated back to the database.",
      category: "performance",
      appliesTo: { kind: "requires-framework", framework: "aspnetcore" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-aspnetcore-ef"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-aspnetcore-async",
      title: "Deadlocks and Thread Pool Starvation",
      body: "Blocking on async code by calling `.Result` ties up a thread pool thread while waiting for a background thread to finish. Under load, this rapidly exhausts the ASP.NET thread pool, hanging the entire application.",
      externalRefs: [
        "https://learn.microsoft.com/en-us/aspnet/core/performance/performance-best-practices#avoid-blocking-calls",
      ],
      limitations: [
        "`.Result`/`.Wait()` inside a console app's `Main` or a one-shot startup/bootstrapping path (no `SynchronizationContext` to deadlock against, and no thread-pool contention concern) is not the thread-pool-starvation hazard this rule targets — the risk is specific to ASP.NET Core request-handling code under load, not blocking calls in general.",
      ],
    },
    {
      id: "know-aspnetcore-di",
      title: "Captive Dependencies",
      body: "A Singleton service is created once. If it takes a Scoped service (like a database context) as a dependency, that Scoped service is never disposed, leading to concurrency errors (like EF Core's `DbContext` being used across threads) and memory leaks.",
      externalRefs: [
        "https://learn.microsoft.com/en-us/dotnet/core/extensions/dependency-injection-guidelines#captive-dependencies",
      ],
      limitations: [
        "Injecting `IServiceScopeFactory` or `IServiceProvider` itself into a Singleton and creating a new scope per-operation inside it is the documented safe pattern — a rule that flags any Scoped-typed dependency reaching a Singleton without distinguishing a factory-mediated scope creation from a direct constructor injection will false-positive on this pattern.",
      ],
    },
    {
      id: "know-aspnetcore-ienumerable",
      title: "Multiple Enumeration",
      body: "`IEnumerable<T>` represents a query or generator, not materialized data. Iterating over it twice means the database query (if using EF) or the generator function will be executed twice.",
      externalRefs: [
        "https://learn.microsoft.com/en-us/dotnet/fundamentals/code-analysis/quality-rules/ca1851",
      ],
      limitations: [
        "An `IEnumerable<T>` backed by an already-materialized `List<T>` or `T[]` (common when a method's return type is declared as the interface for abstraction but the concrete instance is a list) can be enumerated repeatedly with no extra cost — flagging every multiple-enumeration pattern without knowing the concrete backing type overstates the risk.",
      ],
    },
    {
      id: "know-aspnetcore-csrf",
      title: "Cross-Site Request Forgery",
      body: "CSRF allows malicious sites to forge requests to your application using the victim's ambient credentials (cookies). Validating anti-forgery tokens guarantees the request originated from a form generated by your app.",
      externalRefs: ["https://learn.microsoft.com/en-us/aspnet/core/security/anti-request-forgery"],
      limitations: [
        "A pure JSON API consumed only by SPA/mobile clients using bearer-token auth (no cookie-based session) is not vulnerable to CSRF in the classic form-submission sense, so requiring `[ValidateAntiForgeryToken]` there is a false positive — this rule specifically targets cookie-authenticated MVC/Razor Pages form actions.",
      ],
    },
    {
      id: "know-aspnetcore-ef",
      title: "Entity Framework Change Tracker",
      body: "By default, EF Core tracks all materialized entities so it can detect changes on `SaveChanges()`. This tracking consumes significant CPU and memory. Disabling it via `.AsNoTracking()` provides massive performance gains for read-only queries.",
      externalRefs: ["https://learn.microsoft.com/en-us/ef/core/querying/tracking"],
      limitations: [
        "A query whose results will be mutated and saved back in the same request/`DbContext` scope (a typical update flow: fetch, modify properties, call `SaveChanges()`) legitimately needs tracking — applying `.AsNoTracking()` there breaks the update, so this rule only holds for genuinely read-only query paths, not every query indiscriminately.",
      ],
    },
  ],
};
