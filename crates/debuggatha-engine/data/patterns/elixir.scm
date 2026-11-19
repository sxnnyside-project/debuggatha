(call
  target: (identifier) @f (#eq? @f "fragment")
  (arguments (string (interpolation)))) @hit.ecto-fragment-interpolation
(pair key: (keyword) @k (#any-of? @k "check_origin:" "check_origin: ") value: (boolean) @b (#eq? @b "false")) @hit.phoenix-check-origin-off
