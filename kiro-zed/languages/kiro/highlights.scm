[
  (break_keyword)
  (continue_keyword)
  (on_keyword)
  (off_keyword)
  (loop_keyword)
  (in_keyword)
  (per_keyword)
  (return_keyword)
] @keyword

[
  (fn_keyword)
  (pure_keyword)
  (rust_keyword)
  (struct_keyword)
  (var_keyword)
  (import_keyword)
  (error_keyword)
] @keyword

[
  (check_keyword)
  (give_keyword)
  (take_keyword)
  (close_keyword)
  (rest_keyword)
  (run_keyword)
  (ref_keyword)
  (deref_keyword)
  (move_keyword)
] @keyword

[
  (num_type)
  (str_type)
  (bool_type)
  (void_type)
  (adr_type)
  (pipe_type)
  (list_type)
  (map_type)
] @type.builtin

[
  (at_operator)
  (push_operator)
] @operator

[
  (equals_operator)
  (arrow_operator)
  (bang_operator)
  (eq_operator)
  (neq_operator)
  (gte_operator)
  (lte_operator)
  (gt_operator)
  (lt_operator)
  (plus_operator)
  (minus_operator)
  (star_operator)
  (slash_operator)
  (range_operator)
] @operator

(line_comment) @comment
(doc_comment) @comment.doc

(string) @string
(escape_sequence) @string.escape
(number) @number
(true_literal) @boolean
(false_literal) @boolean

(function_definition
  name: (identifier) @function)

(rust_function_declaration
  name: (identifier) @function)

(call_expression
  function: (identifier) @function)

(struct_definition
  name: (type_identifier) @type)

(struct_literal
  type: (type_identifier) @constructor)

(error_definition
  name: (type_identifier) @constant)

(field_definition
  name: (identifier) @property)

(field_initializer
  name: (identifier) @property)

(field_access
  field: (identifier) @property)

(parameter
  name: (identifier) @variable.parameter)

(type_identifier) @type
(identifier) @variable

((identifier) @constant.builtin
  (#match? @constant.builtin "^std_(fs|env|io|net|time)$"))

(len_keyword) @function.builtin
