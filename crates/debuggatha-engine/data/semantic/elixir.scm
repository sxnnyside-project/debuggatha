(call target: (_) @callee (arguments) @args) @call

(anonymous_function) @function
(call
  target: (identifier) @keyword
  (#any-of? @keyword "def" "defp" "defmacro" "defmacrop")) @function

(call target: (identifier) @keyword (#eq? @keyword "for") (arguments) @loop)
(call target: (identifier) @keyword (#eq? @keyword "for") (do_block) @loop)

(call target: (identifier) @keyword (#eq? @keyword "test")) @test

(arguments (string (interpolation)) @dynamic_arg)

(identifier) @ref
(dot right: (identifier) @not_ref)
(binary_operator left: (identifier) @def operator: "=" right: (_) @value)
(stab_clause left: (arguments (identifier) @def))
(call
  target: (identifier) @keyword
  (#any-of? @keyword "def" "defp")
  (arguments (call (arguments (identifier) @def))))
(call
  target: (identifier) @keyword
  (#eq? @keyword "for")
  (arguments (binary_operator left: (identifier) @loop_def operator: "<-")) @loop)

(call target: (_) @callee (arguments (_) @arg) @args) @call
(dot) @path

(call
  target: (identifier) @keyword
  (#any-of? @keyword "alias" "import" "use" "require")
  (arguments (alias) @import))
(call
  target: (identifier) @keyword
  (#any-of? @keyword "alias" "import" "use" "require")
  (arguments (dot left: (alias) @import right: (tuple))))

(call
  target: (identifier) @modifiers
  (#any-of? @modifiers "def" "defp" "defmacro" "defmacrop" "defguard" "defguardp")
  (arguments [(call target: (identifier) @decl @member) (identifier) @decl @member])) @decl_node
(identifier) @mention
