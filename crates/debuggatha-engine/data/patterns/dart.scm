((type_identifier) @hit.avoid-dynamic (#eq? @hit.avoid-dynamic "dynamic"))
(equality_expression
  (member_expression property: (identifier) @p (#eq? @p "length"))
  (equality_operator)
  (decimal_integer_literal) @z
  (#eq? @z "0")) @hit.prefer-is-empty
(null_assertion_expression) @hit.no-force-unwrap
(type_cast_expression) @hit.no-force-cast
(assignment_expression
  left: (assignable_expression property: (identifier) @p (#eq? @p "badCertificateCallback"))) @hit.no-insecure-tls
