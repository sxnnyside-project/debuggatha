(method_invocation arguments: (argument_list) @args) @call

[(method_declaration) (constructor_declaration) (lambda_expression)] @function

(enhanced_for_statement body: (_) @loop)
(for_statement body: (_) @loop)
(while_statement body: (_) @loop)
(do_statement body: (_) @loop)

(method_declaration
  (modifiers (marker_annotation name: (identifier) @annotation))
  (#eq? @annotation "Test")) @test

(argument_list (binary_expression operator: "+") @dynamic_arg)

(identifier) @ref
(field_access field: (identifier) @not_ref)
(method_invocation name: (identifier) @not_ref)
(formal_parameter name: (identifier) @def)
(variable_declarator name: (identifier) @def value: (_) @value)
(catch_formal_parameter name: (identifier) @def)
(lambda_expression parameters: (identifier) @def)
(inferred_parameters (identifier) @def)
(enhanced_for_statement name: (identifier) @loop_def body: (_) @loop)
(for_statement
  init: (local_variable_declaration declarator: (variable_declarator name: (identifier) @loop_def))
  body: (_) @loop)

(method_invocation arguments: (argument_list (_) @arg) @args) @call
(field_access) @path
(assignment_expression left: (identifier) @def right: (_) @value)

(import_declaration (scoped_identifier) @import)

(class_declaration (modifiers)? @modifiers name: (identifier) @decl) @decl_node
(interface_declaration (modifiers)? @modifiers name: (identifier) @decl) @decl_node
(enum_declaration (modifiers)? @modifiers name: (identifier) @decl) @decl_node
(record_declaration (modifiers)? @modifiers name: (identifier) @decl) @decl_node
(method_declaration (modifiers)? @modifiers name: (identifier) @decl @member) @decl_node
[(marker_annotation) (annotation)] @annotation
[(identifier) (type_identifier)] @mention

(interface_body (method_declaration name: (identifier) @decl @member @wired) @decl_node)

(catch_clause body: (block . "{" . "}" .)) @empty_handler
