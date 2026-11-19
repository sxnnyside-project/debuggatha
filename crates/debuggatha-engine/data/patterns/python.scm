(assert_statement) @hit.no-assert-in-production
(except_clause . (block)) @hit.no-bare-except
(default_parameter value: [(list) (dictionary) (set)]) @hit.no-mutable-default-argument
(import_from_statement (wildcard_import)) @hit.no-wildcard-import
(keyword_argument name: (identifier) @k (#eq? @k "verify") value: (false)) @hit.no-insecure-tls
(call
  function: (attribute attribute: (identifier) @f (#eq? @f "run"))
  arguments: (argument_list (keyword_argument name: (identifier) @k (#eq? @k "debug") value: (true)))) @hit.no-flask-debug
(except_clause value: (identifier) @hit.no-catch-all (#any-of? @hit.no-catch-all "Exception" "BaseException"))
(call
  function: (attribute attribute: (identifier) @f (#eq? @f "chmod"))
  arguments: (argument_list (_) (integer) @m (#any-of? @m "0o777" "0o666" "0777"))) @hit.no-permissive-file-mode
(decorator (identifier) @d (#eq? @d "csrf_exempt")) @hit.django-csrf-exempt
(assignment left: (identifier) @n (#eq? @n "DEBUG") right: (true)) @hit.django-debug-enabled
(assignment left: (identifier) @n (#eq? @n "SECRET_KEY") right: (string)) @hit.django-secret-key-in-code
(assignment
  left: (identifier) @n (#eq? @n "ALLOWED_HOSTS")
  right: (list (string (string_content) @s (#eq? @s "*")))) @hit.django-allowed-hosts-wildcard
(assignment
  left: (identifier) @n (#any-of? @n "SESSION_COOKIE_SECURE" "CSRF_COOKIE_SECURE" "SECURE_SSL_REDIRECT")
  right: (false)) @hit.django-cookie-not-secure
(assignment left: (attribute attribute: (identifier) @a (#eq? @a "secret_key")) right: (string)) @hit.flask-secret-key-in-code
(keyword_argument
  name: (identifier) @k (#eq? @k "allow_origins")
  value: (list (string (string_content) @s (#eq? @s "*")))) @hit.no-open-cors
(assignment left: (identifier) @n (#any-of? @n "CORS_ALLOW_ALL_ORIGINS" "CORS_ORIGIN_ALLOW_ALL") right: (true)) @hit.no-open-cors
