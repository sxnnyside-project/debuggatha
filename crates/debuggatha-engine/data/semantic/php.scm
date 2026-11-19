(function_call_expression function: (_) @callee arguments: (arguments) @args) @call
(member_call_expression arguments: (arguments) @args) @call
(scoped_call_expression arguments: (arguments) @args) @call
(exit_statement "exit" @callee) @call

[(function_definition) (method_declaration) (anonymous_function) (arrow_function)] @function

(for_statement body: (_) @loop)
(foreach_statement body: (_) @loop)
(while_statement body: (_) @loop)
(do_statement body: (_) @loop)

(arguments (argument [(encapsed_string (variable_name)) (binary_expression operator: ".")]) @dynamic_arg)

(variable_name) @ref
(simple_parameter name: (variable_name) @def)
(assignment_expression left: (variable_name) @def right: (_) @value)
(foreach_statement (pair (variable_name) @loop_def) body: (_) @loop)
(foreach_statement . (_) . (variable_name) @loop_def body: (_) @loop)
(for_statement initialize: (assignment_expression left: (variable_name) @loop_def) body: (_) @loop)

(function_call_expression function: (_) @callee arguments: (arguments (argument) @arg) @args) @call
(member_call_expression arguments: (arguments (argument) @arg) @args) @call
(scoped_call_expression arguments: (arguments (argument) @arg) @args) @call
(cast_expression (cast_type) @type (#any-of? @type "int" "integer" "float" "double" "bool" "boolean")) @clean

(namespace_use_declaration (namespace_use_clause (qualified_name) @import))
(namespace_use_declaration (namespace_name) @import body: (namespace_use_group))
(require_expression (encapsed_string (string_content) @import))
(require_once_expression (encapsed_string (string_content) @import))
(include_expression (encapsed_string (string_content) @import))
(include_once_expression (encapsed_string (string_content) @import))
((namespace_use_declaration (namespace_use_clause (qualified_name) @not_import)) @declaration
  (#match? @declaration "^use\\s+(function|const)\\b"))

(class_declaration name: (name) @decl) @decl_node
(function_definition name: (name) @decl) @decl_node
(method_declaration (visibility_modifier)? @modifiers name: (name) @decl @member) @decl_node
(attribute_list) @annotation
(name) @mention

(member_call_expression name: [(variable_name) (binary_expression) (encapsed_string) (parenthesized_expression)]) @dynamic_call
(scoped_call_expression name: (variable_name)) @dynamic_call

(catch_clause body: (compound_statement . "{" . "}" .)) @empty_handler
