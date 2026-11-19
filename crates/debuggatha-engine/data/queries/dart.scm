; 0  print(..) as a plain call (not x.print(..), not a declaration)
(call_expression
  function: (identifier) @name
  arguments: (arguments) @args
  (#eq? @name "print"))
