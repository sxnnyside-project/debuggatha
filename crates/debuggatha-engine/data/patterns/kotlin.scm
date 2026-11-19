(unary_expression "!!") @hit.no-double-bang
(catch_block (user_type (identifier) @hit.no-catch-all (#any-of? @hit.no-catch-all "Exception" "Throwable")))
