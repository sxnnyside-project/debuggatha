; Platform.environment['X']
(index_expression
  object: (_) @callee
  index: (string_literal) @value)
; String.fromEnvironment('X')
(call_expression
  function: (_) @callee
  arguments: (arguments . (string_literal) @value))
; const String.fromEnvironment('X')
(const_object_expression
  type: (_) @callee
  arguments: (arguments . (string_literal) @value))
; String.fromEnvironment('X', defaultValue: ..)
(call_expression
  function: (_) @callee
  arguments: (arguments . (string_literal) @value (named_argument) @default))
(const_object_expression
  type: (_) @callee
  arguments: (arguments . (string_literal) @value (named_argument) @default))
; Platform.environment['X'] ?? default
(if_null_expression
  . (index_expression object: (_) @callee index: (string_literal) @value)) @default
