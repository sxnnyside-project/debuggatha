(call_expression function: (_) @callee arguments: (arguments) @args) @call
(macro_invocation macro: (_) @callee_macro (token_tree) @args) @call
(token_tree (identifier) @callee_macro . "!" . (token_tree) @args)

[(function_item) (closure_expression)] @function

(for_expression body: (_) @loop)
(while_expression body: (_) @loop)
(loop_expression body: (_) @loop)

(
  (attribute_item
    (attribute [(identifier) @attr (scoped_identifier name: (identifier) @attr)])
    (#eq? @attr "test"))
  .
  (attribute_item)*
  .
  (function_item) @test)

(
  (attribute_item
    (attribute (identifier) @attr arguments: (token_tree (identifier) @arg))
    (#eq? @attr "cfg")
    (#eq? @arg "test"))
  .
  (attribute_item)*
  .
  [(mod_item) (function_item)] @test)

(identifier) @ref
(parameter pattern: (identifier) @def)
(parameter pattern: (tuple_pattern (identifier) @def))
(let_declaration pattern: (identifier) @def value: (_) @value)
(let_declaration pattern: (tuple_pattern (identifier) @def) value: (_) @value)
(closure_parameters (identifier) @def)
(for_expression pattern: (identifier) @loop_def body: (_) @loop)
(for_expression pattern: (tuple_pattern (identifier) @loop_def) body: (_) @loop)

(call_expression function: (_) @callee arguments: (arguments (_) @arg) @args) @call
(field_expression) @path
(assignment_expression left: (identifier) @def right: (_) @value)
