(call_expression function: (_) @callee arguments: (argument_list) @args) @call

[(function_declaration) (method_declaration) (func_literal)] @function

(for_statement body: (_) @loop)

(identifier) @ref
(keyed_element key: (literal_element (identifier) @not_ref))
(parameter_declaration name: (identifier) @def)
(short_var_declaration left: (expression_list (identifier) @def) right: (expression_list) @value)
(var_spec name: (identifier) @def value: (expression_list) @value)
(for_statement (range_clause left: (expression_list (identifier) @loop_def)) body: (_) @loop)
(for_statement
  (for_clause initializer: (short_var_declaration left: (expression_list (identifier) @loop_def)))
  body: (_) @loop)

(call_expression function: (_) @callee arguments: (argument_list (_) @arg) @args) @call
(selector_expression) @path
(assignment_statement left: (expression_list (identifier) @def) right: (expression_list) @value)

(import_spec path: (interpreted_string_literal (interpreted_string_literal_content) @import))

(function_declaration name: (identifier) @decl) @decl_node
(method_declaration name: (field_identifier) @decl @member) @decl_node
(type_spec name: (type_identifier) @decl) @decl_node
(source_file (var_declaration (var_spec name: (identifier) @decl)))
(source_file (const_declaration (const_spec name: (identifier) @decl)))
[(identifier) (field_identifier) (type_identifier)] @mention
