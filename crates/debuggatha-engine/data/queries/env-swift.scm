; getenv("X"), ProcessInfo.processInfo.environment["X"] (a subscript is a call suffix)
(call_expression
  . (_) @callee
  (call_suffix (value_arguments . (value_argument value: (line_string_literal) @value))))
; getenv("X") ?? default
(nil_coalescing_expression
  value: (call_expression
    . (_) @callee
    (call_suffix (value_arguments . (value_argument value: (line_string_literal) @value))))) @default
