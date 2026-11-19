(call arguments: (argument_list) @args) @call

[(method) (singleton_method) (lambda) (block) (do_block)] @function

(while body: (_) @loop)
(until body: (_) @loop)
(for body: (_) @loop)

(argument_list [(string (interpolation)) (binary operator: "+")] @dynamic_arg)

(identifier) @ref
(call method: (identifier) @not_ref)
(method_parameters (identifier) @def)
(block_parameters (identifier) @def)
(assignment left: (identifier) @def right: (_) @value)
(for pattern: (identifier) @loop_def body: (_) @loop)

(call arguments: (argument_list (_) @arg) @args) @call

(call receiver: (_) method: (identifier) @callee_end block: (_)) @call

(call receiver: (_) method: (identifier) @callee_end !arguments !block) @call

(call
  method: (identifier) @method
  (#any-of? @method "require" "require_relative" "load")
  arguments: (argument_list (string (string_content) @import)))

(program (method name: (identifier) @decl) @decl_node)
(class body: (body_statement (method name: (identifier) @decl @member) @decl_node))
(class name: (constant) @decl) @decl_node
[(identifier) (constant)] @mention

(rescue !body) @empty_handler
