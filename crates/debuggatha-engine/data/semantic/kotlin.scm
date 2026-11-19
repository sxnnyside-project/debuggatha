(call_expression . (_) @callee [(value_arguments) (annotated_lambda)] @args) @call

[(function_declaration) (lambda_literal)] @function

(for_statement (block) @loop)
(while_statement (block) @loop)

(function_declaration
  (modifiers (annotation (user_type (identifier) @annotation)))
  (#eq? @annotation "Test")) @test

(value_arguments (value_argument (binary_expression operator: "+")) @dynamic_arg)

(identifier) @ref
(navigation_expression . (_) (identifier) @not_ref)
(parameter (identifier) @def)
(variable_declaration (identifier) @def)
(property_declaration (variable_declaration (identifier) @def) . (_) @value)
(for_statement (variable_declaration (identifier) @loop_def) (block) @loop)

(call_expression . (_) @callee (value_arguments (value_argument) @arg) @args) @call
(navigation_expression) @path

(import (qualified_identifier) @import)

(source_file (function_declaration (modifiers)? @modifiers name: (identifier) @decl) @decl_node)
(class_declaration (modifiers)? @modifiers name: (identifier) @decl) @decl_node
(object_declaration (modifiers)? @modifiers name: (identifier) @decl) @decl_node
(class_body (function_declaration (modifiers)? @modifiers name: (identifier) @decl @member) @decl_node)
(annotation) @annotation
(identifier) @mention

(catch_block (block . "{" . "}" .)) @empty_handler
