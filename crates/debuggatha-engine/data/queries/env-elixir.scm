; System.get_env("X"), System.fetch_env!("X")
(call
  target: (_) @callee
  (arguments . (string) @value))
; System.get_env("X", default)
(call
  target: (_) @callee
  (arguments . (string) @value . (_) @default))
