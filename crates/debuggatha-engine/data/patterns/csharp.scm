(method_declaration
  (modifier) @m (#eq? @m "async")
  returns: (predefined_type) @r (#eq? @r "void")) @hit.no-async-void
(lock_statement "this") @hit.no-lock-this
((catch_declaration type: (identifier) @hit.no-catch-all) (#eq? @hit.no-catch-all "Exception"))
(catch_clause . (block)) @hit.no-catch-all
(assignment_expression left: (member_access_expression name: (identifier) @n (#eq? @n "ServerCertificateValidationCallback"))) @hit.no-insecure-tls
((identifier) @hit.no-insecure-tls (#eq? @hit.no-insecure-tls "DangerousAcceptAnyServerCertificateValidator"))
(attribute name: (identifier) @n (#eq? @n "IgnoreAntiforgeryToken")) @hit.aspnet-ignore-antiforgery
(object_creation_expression type: (identifier) @t (#eq? @t "HttpClient")) @hit.aspnet-new-httpclient
