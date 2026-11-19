(global_declaration) @hit.no-global-variables
(variable_name (name) @hit.no-global-variables (#eq? @hit.no-global-variables "GLOBALS"))
(error_suppression_expression) @hit.no-error-suppression
(function_call_expression
  function: (name) @f (#eq? @f "chmod")
  arguments: (arguments (argument) (argument (integer) @m (#any-of? @m "0777" "0666" "0o777")))) @hit.no-permissive-file-mode
