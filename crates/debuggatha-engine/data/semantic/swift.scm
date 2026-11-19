(call_expression . (_) @callee (call_suffix (value_arguments) @args)) @call
(call_expression . (_) @callee (call_suffix (lambda_literal) @args)) @call

[(function_declaration) (lambda_literal)] @function

(for_statement (statements) @loop)
(while_statement (statements) @loop)
(repeat_while_statement (statements) @loop)

(import_declaration (identifier) @import)

(catch_block . (catch_keyword) .) @empty_handler

(call_expression . (_) @callee (call_suffix (value_arguments (value_argument) @arg) @args)) @call

(source_file (class_declaration (modifiers)? @modifiers name: (type_identifier) @decl) @decl_node)
(source_file (protocol_declaration (modifiers)? @modifiers name: (type_identifier) @decl) @decl_node)
(source_file (typealias_declaration (modifiers)? @modifiers name: (type_identifier) @decl) @decl_node)
(source_file (function_declaration (modifiers)? @modifiers name: (simple_identifier) @decl) @decl_node)
(source_file (property_declaration (modifiers)? @modifiers name: (pattern bound_identifier: (simple_identifier) @decl)) @decl_node)
(class_body (function_declaration (modifiers)? @modifiers name: (simple_identifier) @decl @member) @decl_node)
[(simple_identifier) (type_identifier)] @mention
