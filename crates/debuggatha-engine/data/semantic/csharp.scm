(invocation_expression function: (_) @callee arguments: (argument_list) @args) @call

[(method_declaration) (constructor_declaration) (local_function_statement) (lambda_expression)] @function

(foreach_statement body: (_) @loop)
(for_statement body: (_) @loop)
(while_statement body: (_) @loop)
(do_statement body: (_) @loop)

(method_declaration
  (attribute_list (attribute name: (identifier) @attribute))
  (#any-of? @attribute "Fact" "Theory" "Test" "TestMethod")) @test

(identifier) @ref
(member_access_expression name: (identifier) @not_ref)
(parameter name: (identifier) @def)
(variable_declarator name: (identifier) @def . (_) @value)
(foreach_statement left: (identifier) @loop_def body: (_) @loop)

(invocation_expression function: (_) @callee arguments: (argument_list (argument) @arg) @args) @call
(member_access_expression) @path
(assignment_expression left: (identifier) @def right: (_) @value)
(lambda_expression parameters: (implicit_parameter) @def)

(using_directive (qualified_name) @import)
(using_directive (identifier) @import .)

(class_declaration (modifier)* @modifiers name: (identifier) @decl @wired) @decl_node
(struct_declaration (modifier)* @modifiers name: (identifier) @decl @wired) @decl_node
(interface_declaration (modifier)* @modifiers name: (identifier) @decl) @decl_node
(enum_declaration (modifier)* @modifiers name: (identifier) @decl) @decl_node
(record_declaration (modifier)* @modifiers name: (identifier) @decl @wired) @decl_node
(method_declaration (modifier)* @modifiers name: (identifier) @decl @member) @decl_node
(attribute_list) @annotation
(identifier) @mention

(interface_declaration body: (declaration_list (method_declaration name: (identifier) @decl @member @wired) @decl_node))

(catch_clause body: (block . "{" . "}" .)) @empty_handler
