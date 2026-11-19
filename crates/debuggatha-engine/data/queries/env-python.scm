; os.getenv("X"), os.environ.get("X")
(call
  function: (_) @callee
  arguments: (argument_list . (string) @value))
; os.environ["X"]
(subscript
  value: (_) @callee
  subscript: (string) @value)
; os.environ["X"] = ".." writes, it does not read
(assignment
  left: (subscript
    value: (_) @w_callee
    subscript: (string) @w_value))
; os.getenv("X", default): the second argument is a default
(call
  function: (_) @callee
  arguments: (argument_list . (string) @value . (_) @default))
; os.environ.get("X") or default
(boolean_operator
  left: (call
    function: (_) @callee
    arguments: (argument_list . (string) @value))
  operator: "or") @default
(boolean_operator
  left: (subscript value: (_) @callee subscript: (string) @value)
  operator: "or") @default
