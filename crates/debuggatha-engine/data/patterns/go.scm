(import_spec path: (interpreted_string_literal (interpreted_string_literal_content) @p (#eq? @p "unsafe"))) @hit.no-unsafe-package
(import_spec path: (interpreted_string_literal (interpreted_string_literal_content) @p (#eq? @p "net/http/pprof"))) @hit.no-pprof-import
(keyed_element
  (literal_element (identifier) @k (#eq? @k "InsecureSkipVerify"))
  (literal_element (true))) @hit.no-insecure-tls
(for_statement body: (block (statement_list (defer_statement) @hit.no-defer-in-loop)))
(call_expression
  function: (selector_expression field: (field_identifier) @f (#any-of? @f "Chmod" "MkdirAll" "Mkdir"))
  arguments: (argument_list (_) (int_literal) @m (#any-of? @m "0777" "0o777" "0666" "0o666"))) @hit.no-permissive-file-mode
