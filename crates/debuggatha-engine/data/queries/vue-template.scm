; Read on the template of a .vue file with the HTML grammar (a template is HTML plus directives).

; 0  an attribute named exactly v-html (the `=` is checked by the reader)
(attribute (attribute_name) @name (#eq? @name "v-html"))
