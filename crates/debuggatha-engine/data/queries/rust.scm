; Patterns are numbered in the order they appear; `detect/queries.rs` reads them by index.

; 0  .unwrap() as a method call (an `.expect(..)` matched here is left to pattern 9)
(call_expression
  function: (field_expression field: (field_identifier) @method)
  arguments: (arguments) @args
  (#any-of? @method "unwrap" "expect"))

; 1  the same inside a macro, where the parser keeps only tokens
(token_tree "." . (identifier) @method . (token_tree) @args
  (#any-of? @method "unwrap" "expect"))

; 2  unsafe { .. }
(unsafe_block) @block

; 3  `unsafe fn` (and `unsafe extern "C" fn`, which the check below leaves out)
(function_modifiers "unsafe" @kw)

; 4  unsafe { .. } inside a macro
(token_tree "unsafe" @kw . (token_tree) @body)

; 5  a function marked #[test] (or #[tokio::test] ..), possibly after other attributes or comments
((attribute_item
   (attribute [(identifier) @attr (scoped_identifier name: (identifier) @attr)])
   (#eq? @attr "test"))
 .
 [(attribute_item) (line_comment) (block_comment)]*
 .
 (function_item) @item)

; 6  an item under #[cfg(test)], with attributes or comments between the attribute and the item
((attribute_item
   (attribute (identifier) @attr arguments: (token_tree (identifier) @arg))
   (#eq? @attr "cfg")
   (#eq? @arg "test"))
 .
 [(attribute_item) (line_comment) (block_comment)]*
 .
 [(mod_item) (function_item)] @item)

; 7  .expect(x) with exactly one argument: Option::expect and Result::expect take one, so a method
;    of the program's own that happens to be called `expect(kind, what)` is not one of them
(call_expression
  function: (field_expression field: (field_identifier) @method)
  arguments: (arguments . (_) .) @args
  (#eq? @method "expect"))
