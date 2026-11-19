; Environment.GetEnvironmentVariable("X")
(invocation_expression
  function: (_) @callee
  arguments: (argument_list . (argument (string_literal) @value)))
; Environment.GetEnvironmentVariable("X") ?? default
(binary_expression
  left: (invocation_expression
    function: (_) @callee
    arguments: (argument_list . (argument (string_literal) @value)))
  operator: "??") @default
