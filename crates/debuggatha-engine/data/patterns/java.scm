(import_declaration (asterisk)) @hit.no-wildcard-import
((method_declaration name: (identifier) @hit.no-finalize) (#eq? @hit.no-finalize "finalize"))
((catch_type (type_identifier) @hit.no-catch-all) (#any-of? @hit.no-catch-all "Exception" "Throwable"))
((type_identifier) @hit.no-insecure-tls (#eq? @hit.no-insecure-tls "NoopHostnameVerifier"))
((identifier) @hit.no-insecure-tls (#eq? @hit.no-insecure-tls "ALLOW_ALL_HOSTNAME_VERIFIER"))
(field_declaration (modifiers (marker_annotation name: (identifier) @a (#eq? @a "Autowired")))) @hit.spring-field-injection
(annotation
  name: (identifier) @a (#eq? @a "CrossOrigin")
  arguments: (annotation_argument_list
    (element_value_pair key: (identifier) @k (#any-of? @k "origins" "value") value: (string_literal (string_fragment) @s (#eq? @s "*"))))) @hit.no-open-cors
