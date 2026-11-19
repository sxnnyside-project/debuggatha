((scope_resolution name: (constant) @hit.no-insecure-tls) (#eq? @hit.no-insecure-tls "VERIFY_NONE"))
((rescue exceptions: (exceptions (constant) @hit.no-catch-all)) (#eq? @hit.no-catch-all "Exception"))
(call
  receiver: (constant) method: (identifier) @f (#eq? @f "chmod")
  arguments: (argument_list . (integer) @m (#any-of? @m "0777" "0666"))) @hit.no-permissive-file-mode
((identifier) @hit.no-csrf-skip (#eq? @hit.no-csrf-skip "skip_forgery_protection"))
(call
  method: (identifier) @m (#eq? @m "skip_before_action")
  arguments: (argument_list (simple_symbol) @s (#any-of? @s ":authenticate_user!" ":verify_authenticity_token" ":authenticate" ":require_login" ":authorize"))) @hit.rails-skip-authentication
