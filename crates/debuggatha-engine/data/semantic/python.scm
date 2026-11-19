(call function: (_) @callee arguments: (argument_list) @args) @call

[(function_definition) (lambda)] @function

(for_statement body: (_) @loop)
(while_statement body: (_) @loop)
[(list_comprehension) (set_comprehension) (dictionary_comprehension) (generator_expression)] @loop

(function_definition name: (identifier) @name (#match? @name "^test")) @test

(argument_list [(string (interpolation)) (binary_operator)] @dynamic_arg)

(identifier) @ref
(attribute attribute: (identifier) @not_ref)
(keyword_argument name: (identifier) @not_ref)
(parameters (identifier) @def)
(default_parameter name: (identifier) @def)
(list_splat_pattern (identifier) @def)
(dictionary_splat_pattern (identifier) @def)
(assignment left: (identifier) @def right: (_) @value)
(assignment left: (pattern_list (identifier) @def) right: (_) @value)
(as_pattern_target (identifier) @def)
(for_statement left: (identifier) @loop_def body: (_) @loop)
(for_statement left: (pattern_list (identifier) @loop_def) body: (_) @loop)

(call function: (_) @callee arguments: (argument_list (_) @arg) @args) @call
(attribute) @path

(import_statement name: (dotted_name) @import)
(import_statement name: (aliased_import name: (dotted_name) @import))
(import_from_statement module_name: (dotted_name) @import)
(import_from_statement module_name: (relative_import) @import)

(module (function_definition name: (identifier) @decl) @decl_node)
(module (class_definition name: (identifier) @decl) @decl_node)
(module (decorated_definition definition: (function_definition name: (identifier) @decl)) @decl_node)
(module (decorated_definition definition: (class_definition name: (identifier) @decl)) @decl_node)
(class_definition body: (block (function_definition name: (identifier) @decl @member) @decl_node))
(class_definition body: (block (decorated_definition definition: (function_definition name: (identifier) @decl @member)) @decl_node))
(decorator) @annotation
(identifier) @mention

(except_clause (block . (pass_statement) .)) @empty_handler
