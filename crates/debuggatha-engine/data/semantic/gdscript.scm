(call (_) @callee arguments: (arguments) @args) @call
(attribute_call (_) @callee arguments: (arguments) @args) @call

[(function_definition) (lambda)] @function

(for_statement body: (_) @loop)
(while_statement body: (_) @loop)
