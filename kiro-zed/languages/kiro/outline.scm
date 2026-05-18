(function_definition
  name: (identifier) @name) @item

(rust_function_declaration
  name: (identifier) @name) @item

(struct_definition
  name: (type_identifier) @name) @item

(error_definition
  name: (type_identifier) @name) @item

(documented_item
  (doc_comment) @annotation)
