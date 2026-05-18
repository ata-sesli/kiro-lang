(function_definition
  body: (block
    "{"
    (_)* @function.inside
    "}")) @function.around

(rust_function_declaration) @function.around

(struct_definition
  body: (field_block
    "{"
    (_)* @class.inside
    "}")) @class.around

(line_comment)+ @comment.around
(doc_comment)+ @comment.around
