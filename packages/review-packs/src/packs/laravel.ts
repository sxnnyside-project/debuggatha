import type { ReviewPack } from "@debuggatha/knowledge-system";

export const laravelPack: ReviewPack = {
  id: "debuggatha/laravel",
  version: "1.0.0",
  kind: "stack",
  displayName: "Laravel",
  dependsOn: [],
  rules: [
    {
      id: "mass-assignment-protection",
      packId: "debuggatha/laravel",
      statement:
        "Always define `$fillable` or `$guarded` on Eloquent models to prevent Mass Assignment vulnerabilities.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "laravel" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-laravel-mass-assignment"],
      contradicts: undefined,
    },
    {
      id: "n-plus-one",
      packId: "debuggatha/laravel",
      statement:
        "Use `with()` to eager load Eloquent relationships when iterating over a collection of models.",
      category: "performance",
      appliesTo: { kind: "requires-framework", framework: "laravel" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-laravel-n-plus-one"],
      contradicts: undefined,
    },
    {
      id: "form-requests",
      packId: "debuggatha/laravel",
      statement:
        "Use FormRequest classes for complex validation instead of calling `$request->validate()` directly in the controller.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "laravel" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-laravel-form-requests"],
      contradicts: undefined,
    },
    {
      id: "no-env-outside-config",
      packId: "debuggatha/laravel",
      statement:
        "Never call the `env()` helper outside of configuration files (`config/*.php`). Use `config('key')` elsewhere.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "laravel" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-laravel-env"],
      contradicts: undefined,
    },
    {
      id: "blade-escaping",
      packId: "debuggatha/laravel",
      statement:
        "Use `{{ $var }}` for echoing data in Blade. Only use `{!! $var !!}` when explicitly rendering trusted, pre-sanitized HTML.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "laravel" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-laravel-blade-xss"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-laravel-mass-assignment",
      title: "Mass Assignment Vulnerability",
      body: "If a user submits a payload like `['is_admin' => true]` and it is passed directly to `Model::create($request->all())`, the model will be elevated unless `$fillable` strictly limits which attributes can be mass-assigned.",
      externalRefs: ["https://laravel.com/docs/eloquent#mass-assignment"],
    },
    {
      id: "know-laravel-n-plus-one",
      title: "N+1 Query Problem",
      body: "Accessing a relationship inside a loop on a collection of models triggers a new SQL query for every single item in the loop. Eager loading via `with('relation')` executes exactly two queries regardless of collection size.",
      externalRefs: ["https://laravel.com/docs/eloquent-relationships#eager-loading"],
    },
    {
      id: "know-laravel-form-requests",
      title: "Fat Controllers",
      body: "Putting heavy validation arrays directly in the controller breaks the single responsibility principle and clutters the routing logic. FormRequests decouple authorization and validation.",
      externalRefs: ["https://laravel.com/docs/validation#form-request-validation"],
    },
    {
      id: "know-laravel-env",
      title: "Configuration Caching",
      body: "When `php artisan config:cache` is executed in production, the `env()` function returns `null` for all calls to prevent disk reads. Calling `env()` in controllers or services will cause the app to fail in production.",
      externalRefs: ["https://laravel.com/docs/configuration#configuration-caching"],
    },
    {
      id: "know-laravel-blade-xss",
      title: "Unescaped Blade Output",
      body: "The `{{ }}` syntax automatically passes data through PHP's `htmlspecialchars` function. The `{!! !!}` syntax bypasses this, creating an immediate XSS vulnerability if the data is user-supplied.",
      externalRefs: ["https://laravel.com/docs/blade#displaying-data"],
    },
  ],
};
