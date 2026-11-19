; std::env::var("X"), env::var_os("X")
(call_expression
  function: (_) @callee
  arguments: (arguments . (string_literal) @value))
; env!("X"), option_env!("X"): the arguments of a macro are tokens
(macro_invocation
  macro: (_) @callee
  (token_tree . (string_literal) @value))
; env::var("X").unwrap_or(..), .ok(), ...: the code handles the variable being absent
(call_expression
  function: (field_expression
    value: (call_expression
      function: (_) @callee
      arguments: (arguments . (string_literal) @value))
    field: (field_identifier) @fallback))
; env! nested in another macro or in an attribute: include_str!(env!("X")), #[command(version = env!("X"))]
(token_tree
  (identifier) @callee
  .
  "!"
  .
  (token_tree . (string_literal) @value))
