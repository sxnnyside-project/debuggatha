(try_expression (try_operator) @op (#eq? @op "try!")) @hit.no-force-try
(as_expression (as_operator) @op (#eq? @op "as!")) @hit.no-force-cast
(postfix_expression operation: (bang)) @hit.no-force-unwrap
