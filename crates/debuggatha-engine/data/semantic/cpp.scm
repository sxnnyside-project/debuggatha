(call_expression function: (_) @callee arguments: (argument_list) @args) @call

[(function_definition) (lambda_expression)] @function

(for_statement body: (_) @loop)
(for_range_loop body: (_) @loop)
(while_statement body: (_) @loop)
(do_statement body: (_) @loop)

(catch_clause body: (compound_statement . "{" . "}" .)) @empty_handler

(call_expression function: (_) @callee arguments: (argument_list (_) @arg) @args) @call
