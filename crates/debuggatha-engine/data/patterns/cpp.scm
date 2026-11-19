(new_expression) @hit.no-raw-new-delete
(delete_expression) @hit.no-raw-new-delete
((call_expression function: (identifier) @hit.no-malloc-free)
  (#any-of? @hit.no-malloc-free "malloc" "calloc" "realloc" "free"))
(using_declaration "namespace") @hit.no-using-namespace-in-header
(cast_expression) @hit.no-c-style-cast
(catch_clause parameters: (parameter_list "...")) @hit.no-catch-all
(call_expression function: (template_function name: (identifier) @n (#any-of? @n "reinterpret_cast" "const_cast"))) @hit.no-type-punning
