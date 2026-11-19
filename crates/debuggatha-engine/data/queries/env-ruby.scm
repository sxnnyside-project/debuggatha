; ENV["X"]
(element_reference
  object: (_) @callee
  . (string) @value)
; ENV.fetch("X")
(call
  receiver: (_) @callee
  method: (identifier) @method
  arguments: (argument_list . (string) @value))
; ENV["X"] = ".." writes, it does not read
(assignment
  left: (element_reference
    object: (_) @w_callee
    (string) @w_value))
; ENV.fetch("X", default) and ENV.fetch("X") { default }
(call
  receiver: (_) @callee
  method: (identifier) @method
  arguments: (argument_list . (string) @value . (_) @default))
(call
  receiver: (_) @callee
  method: (identifier) @method
  arguments: (argument_list . (string) @value)
  block: (_) @default)
; ENV["X"] || default
(binary
  left: (element_reference object: (_) @callee . (string) @value)
  operator: "||") @default
(binary
  left: (call
    receiver: (_) @callee
    method: (identifier) @method
    arguments: (argument_list . (string) @value))
  operator: "||") @default
