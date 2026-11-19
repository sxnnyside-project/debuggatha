; getenv('X'), env('X')
(function_call_expression
  function: (_) @callee
  arguments: (arguments . (argument [(string) (encapsed_string)] @value)))
; $_ENV['X']
(subscript_expression
  (_) @callee
  [(string) (encapsed_string)] @value)
(assignment_expression
  left: (subscript_expression
    (_) @w_callee
    [(string) (encapsed_string)] @w_value))
; env('X', default)
(function_call_expression
  function: (_) @callee
  arguments: (arguments . (argument [(string) (encapsed_string)] @value) . (argument) @default))
; $_ENV['X'] ?? default, getenv('X') ?: default, env('X') ?? default
(binary_expression
  left: (subscript_expression (_) @callee [(string) (encapsed_string)] @value)
  operator: _ @op
  (#any-of? @op "??" "?:")) @default
(binary_expression
  left: (function_call_expression
    function: (_) @callee
    arguments: (arguments . (argument [(string) (encapsed_string)] @value)))
  operator: _ @op
  (#eq? @op "??")) @default
(conditional_expression
  condition: (function_call_expression
    function: (_) @callee
    arguments: (arguments . (argument [(string) (encapsed_string)] @value)))) @default
