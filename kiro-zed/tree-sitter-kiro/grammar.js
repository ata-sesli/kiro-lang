const PREC = {
  ASSIGN: -1,
  RANGE: 0,
  COMPARE: 1,
  ADD: 2,
  MULTIPLY: 3,
  PREFIX: 4,
  CALL: 5,
  ACCESS: 6,
};

module.exports = grammar({
  name: "kiro",

  extras: $ => [
    /[\s\uFEFF\u2060\u200B]/,
    $.line_comment,
  ],

  word: $ => $.identifier,

  conflicts: $ => [
    [$.expression, $._call_target],
  ],

  rules: {
    source_file: $ => repeat($._statement),

    _statement: $ => choice(
      $.documented_item,
      $.struct_definition,
      $.error_definition,
      $.function_definition,
      $.rust_function_declaration,
      $.variable_declaration,
      $.assignment_statement,
      $.on_statement,
      $.loop_statement,
      $.give_statement,
      $.close_statement,
      $.return_statement,
      $.break_statement,
      $.continue_statement,
      $.rest_statement,
      $.check_statement,
      $.import_statement,
      $.print_statement,
      $.expression_statement,
    ),

    documented_item: $ => seq(
      repeat1($.doc_comment),
      choice($.struct_definition, $.function_definition, $.rust_function_declaration),
    ),

    struct_definition: $ => seq(
      $.struct_keyword,
      field("name", $.type_identifier),
      field("body", $.field_block),
    ),

    field_block: $ => seq(
      "{",
      repeat($.field_definition),
      "}",
    ),

    field_definition: $ => seq(
      field("name", $.identifier),
      ":",
      field("type", $.type),
    ),

    error_definition: $ => seq(
      $.error_keyword,
      field("name", $.type_identifier),
      optional(seq($.equals_operator, field("message", $.string))),
    ),

    function_definition: $ => seq(
      optional($.pure_keyword),
      $.fn_keyword,
      field("name", $.identifier),
      field("parameters", $.parameters),
      optional(seq($.arrow_operator, field("return_type", $.type), optional($.bang_operator))),
      field("body", $.block),
    ),

    rust_function_declaration: $ => seq(
      $.rust_keyword,
      $.fn_keyword,
      field("name", $.identifier),
      field("parameters", $.parameters),
      $.arrow_operator,
      field("return_type", $.type),
      optional($.bang_operator),
    ),

    parameters: $ => seq(
      "(",
      optional(seq($.parameter, repeat(seq(",", $.parameter)), optional(","))),
      ")",
    ),

    parameter: $ => seq(
      field("name", $.identifier),
      ":",
      field("type", $.type),
    ),

    block: $ => seq(
      "{",
      repeat($._statement),
      "}",
    ),

    variable_declaration: $ => seq(
      $.var_keyword,
      field("name", $.identifier),
      $.equals_operator,
      field("value", $.expression),
    ),

    assignment_statement: $ => prec.right(PREC.ASSIGN, seq(
      field("left", $._assignment_target),
      $.equals_operator,
      field("right", $.expression),
    )),

    _assignment_target: $ => choice(
      $.identifier,
      $.field_access,
      $.deref_expression,
    ),

    on_statement: $ => prec.right(seq(
      $.on_keyword,
      "(",
      field("condition", $.expression),
      ")",
      field("body", $.block),
      optional($.off_clause),
      repeat($.error_clause),
    )),

    off_clause: $ => seq($.off_keyword, field("body", $.block)),

    error_clause: $ => seq(
      $.error_keyword,
      optional(field("type", $.type_identifier)),
      field("body", $.block),
    ),

    loop_statement: $ => seq(
      $.loop_keyword,
      choice(
        seq($.on_keyword, "(", field("condition", $.expression), ")", field("body", $.block)),
        seq(
          field("iterator", $.identifier),
          $.in_keyword,
          field("iterable", $.expression),
          optional($.step_clause),
          optional($.loop_filter),
          field("body", $.block),
          optional($.off_clause),
        ),
      ),
    ),

    step_clause: $ => seq($.per_keyword, field("value", $.expression)),

    loop_filter: $ => seq(
      $.on_keyword,
      "(",
      field("condition", $.expression),
      ")",
    ),

    give_statement: $ => seq(
      $.give_keyword,
      field("pipe", $.expression),
      field("value", $.expression),
    ),

    close_statement: $ => seq($.close_keyword, field("pipe", $.expression)),

    return_statement: $ => prec.right(seq($.return_keyword, optional($.expression))),

    break_statement: $ => $.break_keyword,

    continue_statement: $ => $.continue_keyword,

    rest_statement: $ => $.rest_keyword,

    check_statement: $ => seq(
      $.check_keyword,
      field("condition", $.expression),
      optional(seq(",", field("message", $.string))),
    ),

    import_statement: $ => seq($.import_keyword, field("module", $.identifier)),

    print_statement: $ => seq($.print_keyword, field("value", $.expression)),

    expression_statement: $ => $.expression,

    expression: $ => choice(
      $.binary_expression,
      $.range_expression,
      $.call_expression,
      $.run_expression,
      $.field_access,
      $.at_expression,
      $.push_expression,
      $.move_expression,
      $.ref_expression,
      $.deref_expression,
      $.take_expression,
      $.len_expression,
      $.list_literal,
      $.map_literal,
      $.struct_literal,
      $.pipe_literal,
      $.adr_literal,
      $.parenthesized_expression,
      $.number,
      $.string,
      $.boolean,
      $.identifier,
      $.type_identifier,
    ),

    binary_expression: $ => choice(
      ...[
        [$.star_operator, PREC.MULTIPLY],
        [$.slash_operator, PREC.MULTIPLY],
        [$.plus_operator, PREC.ADD],
        [$.minus_operator, PREC.ADD],
        [$.eq_operator, PREC.COMPARE],
        [$.neq_operator, PREC.COMPARE],
        [$.gte_operator, PREC.COMPARE],
        [$.lte_operator, PREC.COMPARE],
        [$.gt_operator, PREC.COMPARE],
        [$.lt_operator, PREC.COMPARE],
      ].map(([operator, precedence]) => prec.left(precedence, seq(
        field("left", $.expression),
        field("operator", operator),
        field("right", $.expression),
      ))),
    ),

    range_expression: $ => prec.left(PREC.RANGE, seq(
      field("start", $.expression),
      $.range_operator,
      field("end", $.expression),
    )),

    call_expression: $ => prec.left(PREC.CALL, seq(
      field("function", $._call_target),
      field("arguments", $.arguments),
    )),

    _call_target: $ => choice($.identifier, $.field_access, $.parenthesized_expression),

    arguments: $ => seq(
      "(",
      optional(seq($.expression, repeat(seq(",", $.expression)), optional(","))),
      ")",
    ),

    run_expression: $ => prec.right(PREC.PREFIX, seq($.run_keyword, field("call", $.expression))),

    field_access: $ => prec.left(PREC.ACCESS, seq(
      field("object", $.expression),
      ".",
      field("field", $.identifier),
    )),

    at_expression: $ => prec.left(PREC.ACCESS, seq(
      field("collection", $.expression),
      $.at_operator,
      field("key", $.expression),
    )),

    push_expression: $ => prec.left(PREC.ACCESS, seq(
      field("list", $.expression),
      $.push_operator,
      field("value", $.expression),
    )),

    move_expression: $ => prec.right(PREC.PREFIX, seq($.move_keyword, field("value", $.identifier))),

    ref_expression: $ => prec.right(PREC.PREFIX, seq($.ref_keyword, field("value", $.expression))),

    deref_expression: $ => prec.right(PREC.PREFIX, seq($.deref_keyword, field("value", $.expression))),

    take_expression: $ => prec.right(PREC.PREFIX, seq($.take_keyword, field("pipe", $.expression))),

    len_expression: $ => prec.right(PREC.PREFIX, seq($.len_keyword, field("value", $.expression))),

    list_literal: $ => prec(PREC.PREFIX, seq(
      $.list_type,
      field("type", $.type),
      "{",
      optional(seq($.expression, repeat(seq(",", $.expression)), optional(","))),
      "}",
    )),

    map_literal: $ => prec(PREC.PREFIX, seq(
      $.map_type,
      field("key_type", $.type),
      field("value_type", $.type),
      "{",
      optional(seq($.map_pair, repeat(seq(",", $.map_pair)), optional(","))),
      "}",
    )),

    map_pair: $ => seq(field("key", $.expression), field("value", $.expression)),

    struct_literal: $ => prec(PREC.PREFIX, seq(
      field("type", $.type_identifier),
      "{",
      optional(seq($.field_initializer, repeat(seq(",", $.field_initializer)), optional(","))),
      "}",
    )),

    field_initializer: $ => seq(
      field("name", $.identifier),
      ":",
      field("value", $.expression),
    ),

    pipe_literal: $ => prec.right(PREC.PREFIX, seq($.pipe_type, field("type", $.type), optional($.number))),

    adr_literal: $ => prec(PREC.PREFIX, seq($.adr_type, field("type", $.type))),

    parenthesized_expression: $ => seq("(", $.expression, ")"),

    type: $ => choice(
      $.num_type,
      $.str_type,
      $.bool_type,
      $.void_type,
      $.type_identifier,
      seq($.adr_type, $.type),
      seq($.pipe_type, $.type),
      seq($.list_type, $.type),
      seq($.map_type, $.type, $.type),
      $.function_type,
    ),

    function_type: $ => prec.right(seq(
      $.fn_keyword,
      "(",
      optional(seq($.type, repeat(seq(",", $.type)), optional(","))),
      ")",
      $.arrow_operator,
      $.type,
      optional($.bang_operator),
    )),

    boolean: $ => choice($.true_literal, $.false_literal),

    number: _ => /\d+(\.\d+)?/,

    string: $ => seq(
      "\"",
      repeat(choice($.escape_sequence, $.string_content)),
      "\"",
    ),

    string_content: _ => token.immediate(prec(1, /[^"\\]+/)),

    escape_sequence: _ => token.immediate(seq("\\", /./)),

    identifier: _ => /[a-z_][a-zA-Z0-9_]*/,

    type_identifier: _ => /[A-Z][a-zA-Z0-9_]*/,

    doc_comment: _ => token(seq("///", /.*/)),

    line_comment: _ => token(seq("//", optional(seq(/[^/]/, /.*/)))),

    break_keyword: _ => "break",
    continue_keyword: _ => "continue",
    on_keyword: _ => "on",
    off_keyword: _ => "off",
    loop_keyword: _ => "loop",
    in_keyword: _ => "in",
    per_keyword: _ => "per",
    return_keyword: _ => "return",
    fn_keyword: _ => "fn",
    pure_keyword: _ => "pure",
    rust_keyword: _ => "rust",
    struct_keyword: _ => "struct",
    var_keyword: _ => "var",
    import_keyword: _ => "import",
    error_keyword: _ => "error",
    check_keyword: _ => "check",
    give_keyword: _ => "give",
    take_keyword: _ => "take",
    close_keyword: _ => "close",
    rest_keyword: _ => "rest",
    run_keyword: _ => "run",
    ref_keyword: _ => "ref",
    deref_keyword: _ => "deref",
    move_keyword: _ => "move",
    print_keyword: _ => "print",
    len_keyword: _ => "len",

    num_type: _ => "num",
    str_type: _ => "str",
    bool_type: _ => "bool",
    void_type: _ => "void",
    adr_type: _ => "adr",
    pipe_type: _ => "pipe",
    list_type: _ => "list",
    map_type: _ => "map",

    true_literal: _ => "true",
    false_literal: _ => "false",

    at_operator: _ => "at",
    push_operator: _ => "push",
    equals_operator: _ => "=",
    arrow_operator: _ => "->",
    bang_operator: _ => "!",
    eq_operator: _ => "==",
    neq_operator: _ => "!=",
    gte_operator: _ => ">=",
    lte_operator: _ => "<=",
    gt_operator: _ => ">",
    lt_operator: _ => "<",
    plus_operator: _ => "+",
    minus_operator: _ => "-",
    star_operator: _ => "*",
    slash_operator: _ => "/",
    range_operator: _ => "..",
  },
});
