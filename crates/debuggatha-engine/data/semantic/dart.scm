(call_expression function: (_) @callee arguments: (arguments) @args) @call

[(function_declaration) (function_expression)] @function

(for_statement body: (_) @loop)
(while_statement body: (_) @loop)
(do_statement body: (_) @loop)

(identifier) @ref
(member_expression property: (identifier) @not_ref)
(formal_parameter name: (identifier) @def)
(initialized_variable_declaration name: (identifier) @def value: (_) @value)
(for_statement name: (identifier) @loop_def body: (_) @loop)
(for_statement
  init: (local_variable_declaration (initialized_variable_declaration name: (identifier) @loop_def))
  body: (_) @loop)

(call_expression function: (_) @callee arguments: (arguments (_) @arg) @args) @call
(member_expression) @path
(formal_parameter (identifier) @def)

(configurable_uri (uri) @import)
(part_directive uri: (uri) @import)

(function_declaration signature: (function_signature name: (identifier) @decl)) @decl_node
(class_declaration name: (identifier) @decl) @decl_node
(method_declaration signature: (method_signature (function_signature name: (identifier) @decl @member))) @decl_node
(annotation) @annotation
[(identifier) (type_identifier)] @mention

(try_statement (catch_clause) . (block . "{" . "}" .) @empty_handler)
