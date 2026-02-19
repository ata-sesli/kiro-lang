#[rust_sitter::grammar("kiro")]
pub mod grammar {
    #[rust_sitter::language]
    pub struct Program {
        pub statements: Vec<Statement>,
    }
    // 1. The Wrapper Struct
    #[derive(Debug, Clone)]
    pub struct NumberVal {
        #[rust_sitter::leaf(pattern = r"\d+(\.\d+)?", transform = |s| s.to_string())]
        pub value: String,
    }
    #[derive(Debug, Clone)]
    pub struct VariableVal {
        #[rust_sitter::leaf(pattern = r"[a-z_][a-zA-Z0-9_]*", transform = |s| s.to_string())]
        pub value: String,
    }

    // 2. Wrapper for String Literals ("hello")
    #[derive(Debug, Clone)]
    pub struct StringVal {
        #[rust_sitter::leaf(pattern = r#""([^"\\]|\\.)*""#, transform = |s| s.to_string())]
        pub value: String,
    }
    // 3. For Struct Names (Capitalized: "User")
    #[derive(Debug, Clone)]
    pub struct StructNameVal {
        #[rust_sitter::leaf(pattern = r"[A-Z][a-zA-Z0-9_]*", transform = |s| s.to_string())]
        pub value: String,
    }

    // 4. For Field Names (Lowercase: "age")
    #[derive(Debug, Clone)]
    pub struct FieldNameVal {
        #[rust_sitter::leaf(pattern = r"[a-z_]+", transform = |s| s.to_string())]
        pub value: String,
    }

    #[derive(Debug, Clone)]
    pub enum BoolVal {
        True(#[rust_sitter::leaf(text = "true")] ()),
        False(#[rust_sitter::leaf(text = "false")] ()),
    }
    #[derive(Debug, Clone)]
    pub enum KiroType {
        /// Numeric type.
        #[rust_sitter::leaf(text = "num")]
        Num, // Replaces Int
        /// String/text type.
        #[rust_sitter::leaf(text = "str")]
        Str, // New
        /// Boolean type (`true` / `false`).
        #[rust_sitter::leaf(text = "bool")]
        Bool, // New
        /// No-value type for procedures.
        #[rust_sitter::leaf(text = "void")]
        Void,
        /// Pointer/address type: `adr <type>`.
        #[rust_sitter::leaf(text = "adr")]
        Adr(#[rust_sitter::leaf(text = "adr")] (), Box<KiroType>),
        /// Channel/pipe type: `pipe <type>`.
        #[rust_sitter::leaf(text = "pipe")]
        Pipe(#[rust_sitter::leaf(text = "pipe")] (), Box<KiroType>),

        // 1. Recursive Types for Collections
        // list <type>
        /// List type: `list <type>`.
        List(#[rust_sitter::leaf(text = "list")] (), Box<KiroType>),
        // map <key_type> <val_type>
        /// Map type: `map <key_type> <value_type>`.
        Map(
            #[rust_sitter::leaf(text = "map")] (),
            Box<KiroType>,
            Box<KiroType>,
        ),
        /// Function type: `fn(type, ...) -> type` (optional `!`).
        FnType(
            #[rust_sitter::leaf(text = "fn")] (),
            #[rust_sitter::leaf(text = "(")] (),
            #[rust_sitter::delimited(#[rust_sitter::leaf(text = ",")] ())] Vec<KiroType>,
            #[rust_sitter::leaf(text = ")")] (),
            #[rust_sitter::leaf(text = "->")] (),
            Box<KiroType>,
        ),

        // 1. Custom Types (e.g., "User")
        // We use a high priority to ensure it doesn't conflict with keywords
        Custom(StructNameVal),
    }

    // --- MAP PAIR (No colon, just space) ---
    // "Key Value"
    #[derive(Debug, Clone)]
    pub struct MapPair {
        pub key: Expression,
        pub value: Expression,
    }

    #[derive(Debug, Clone)]
    pub struct FuncParam {
        #[rust_sitter::leaf(pattern = r"[a-z_]+", transform = |s| s.to_string())]
        pub name: String,
        #[rust_sitter::leaf(text = ":")]
        _colon: (),
        pub command_type: KiroType,
    }
    // A single field in a struct definition: "name: str"
    #[derive(Debug, Clone)]
    pub struct FieldDef {
        pub name: FieldNameVal,
        #[rust_sitter::leaf(text = ":")]
        _colon: (),
        pub field_type: KiroType,
    }

    // A single field assignment in initialization: "name: 'Kiro'"
    #[derive(Debug, Clone)]
    pub struct FieldInit {
        pub name: FieldNameVal,
        #[rust_sitter::leaf(text = ":")]
        _colon: (),
        pub value: Expression,
    }

    #[derive(Debug, Clone)]
    pub enum Statement {
        // ... (Keep existing Statements) ...

        // 2. Struct Definition (No commas, whitespace separated)
        // struct User { name: str age: num }
        // 2. Struct Definition (No commas, whitespace separated)
        // struct User { name: str age: num }
        /// Struct definition: `struct User { name: str age: num }`.
        StructDef(StructDef),
        /// Error declaration: `error NotFound = "Description"`.
        // Error Definition: error NotFound = "Description"
        ErrorDef {
            #[rust_sitter::leaf(text = "error")]
            _error: (),
            #[rust_sitter::leaf(pattern = r"[A-Z][a-zA-Z0-9_]*", transform = |s| s.to_string())]
            name: String,
            description: Option<ErrorDesc>,
        },
        /// Mutable variable declaration: `var x = expr`.
        // 1. Variable Declaration: var x = 10
        VarDecl {
            #[rust_sitter::leaf(text = "var")]
            _var: (),
            #[rust_sitter::leaf(pattern = r"[a-z_]+", transform = |s| s.to_string())]
            ident: String,
            #[rust_sitter::leaf(text = "=")]
            _eq: (),
            value: Expression,
        },
        /// Assignment/mutation statement: `x = expr` or `obj.field = expr`.
        // 2. Assignment (Mutation): x = 10 OR x.y = 10
        AssignStmt {
            lhs: Expression,
            #[rust_sitter::leaf(text = "=")]
            _eq: (),
            rhs: Expression,
        },
        /// Conditional statement: `on (cond) { ... } off { ... }`.
        #[rust_sitter::prec_right(1)]
        On {
            #[rust_sitter::leaf(text = "on")]
            _on: (),
            #[rust_sitter::leaf(text = "(")]
            _l: (),
            condition: Expression,
            #[rust_sitter::leaf(text = ")")]
            _r: (),
            body: Block,
            // The 'off' part is optional
            else_clause: Option<OffClause>,
            // Multiple error handlers
            error_clauses: Option<ErrorClauseList>,
        },
        /// While-style loop: `loop on (cond) { ... }`.
        LoopOn {
            #[rust_sitter::leaf(text = "loop")]
            _loop: (),
            #[rust_sitter::leaf(text = "on")]
            _on: (),
            #[rust_sitter::leaf(text = "(")]
            _l: (),
            condition: Expression,
            #[rust_sitter::leaf(text = ")")]
            _r: (),
            body: Block,
        },

        /// Iterator loop: `loop item in iterable [per n] [on (filter)] { ... }`.
        // 4. The "For" Loop: loop x in y [per z] [on (cond)] { } [off { }]
        LoopIter {
            #[rust_sitter::leaf(text = "loop")]
            _loop: (),
            #[rust_sitter::leaf(pattern = r"[a-z_]+", transform = |s| s.to_string())]
            iterator: String,
            #[rust_sitter::leaf(text = "in")]
            _in: (),
            iterable: Expression, // This handles 'arr' or '0..10'

            step: Option<StepClause>,   // Optional "per 5"
            filter: Option<LoopFilter>, // Optional "on (x % 2 == 0)"

            body: Block,

            // Optional "off" block for the filter
            else_clause: Option<OffClause>,
        },
        /// Function definition: `pure fn` or `fn`.
        FunctionDef(FunctionDef),
        /// Rust host declaration: `rust fn name(...) -> type[!]`.
        // Rust-backed function declaration (no body)
        // Arrow and return type are REQUIRED to avoid grammar ambiguity
        // Use `rust fn foo() -> void` for functions with no return
        // Rust-backed function declaration (no body)
        // Arrow and return type are REQUIRED to avoid grammar ambiguity
        // Use `rust fn foo() -> void` for functions with no return
        RustFnDecl(RustFnDecl),
        /// Send a value into a pipe: `give ch value`.
        // 1. Give: give <channel> <value>
        Give(
            #[rust_sitter::leaf(text = "give")] (),
            Expression, // Channel
            Expression, // Value
        ),

        /// Close a pipe sender: `close ch`.
        // 2. Close: close <channel>
        Close(
            #[rust_sitter::leaf(text = "close")] (),
            Expression, // Channel
        ),
        /// Return from current function.
        // 3. Return Statement
        #[rust_sitter::prec_right(1)]
        Return(#[rust_sitter::leaf(text = "return")] (), Option<Expression>),
        /// Break from current loop.
        // 4. Break Statement
        Break(#[rust_sitter::leaf(text = "break")] ()),
        /// Continue to next loop iteration.
        // 5. Continue Statement
        Continue(#[rust_sitter::leaf(text = "continue")] ()),

        /// Import module by name: `import math`.
        // 6. Import Statement
        Import {
            #[rust_sitter::leaf(text = "import")]
            _import: (),
            #[rust_sitter::leaf(pattern = r"[a-z_]+", transform = |s| s.to_string())]
            module_name: String,
        },

        /// Expression as statement.
        ExprStmt(Expression),
        /// Print statement: `print expr`.
        Print(#[rust_sitter::leaf(text = "print")] (), Expression),

        /// Item preceded by one or more documentation comments (`///`).
        // Documented Item
        Documented {
            #[rust_sitter::repeat(non_empty = true)]
            doc: Vec<DocComment>,
            item: AnnotatableItem,
        },
    }
    #[derive(Debug, Clone)]
    pub enum Expression {
        // 3. Struct Initialization
        // User { name: "Kiro", age: 10 }
        /// Struct initialization: `User { name: "A", age: 1 }`.
        #[rust_sitter::prec_left(5)]
        StructInit(
            StructNameVal, // Struct Name
            #[rust_sitter::leaf(text = "{")] (),
            #[rust_sitter::delimited(
                #[rust_sitter::leaf(text = ",")] ()
            )]
            Vec<FieldInit>,
            #[rust_sitter::leaf(text = "}")] (),
        ),

        // 2. List Initialization
        // list num { 1, 2, 3 }
        /// List initialization: `list num { ... }`.
        #[rust_sitter::prec_left(2)]
        ListInit(
            #[rust_sitter::leaf(text = "list")] (),
            #[allow(dead_code)] KiroType, // The inner type (e.g. num)
            #[rust_sitter::leaf(text = "{")] (),
            #[rust_sitter::delimited(#[rust_sitter::leaf(text = ",")] ())] Vec<Expression>,
            #[rust_sitter::leaf(text = "}")] (),
        ),

        // 3. Map Initialization
        // map str num { "A" 1, "B" 2 }
        /// Map initialization: `map str num { ... }`.
        #[rust_sitter::prec_left(2)]
        MapInit(
            #[rust_sitter::leaf(text = "map")] (),
            #[allow(dead_code)] KiroType, // Key Type
            #[allow(dead_code)] KiroType, // Value Type
            #[rust_sitter::leaf(text = "{")] (),
            #[rust_sitter::delimited(#[rust_sitter::leaf(text = ",")] ())] Vec<MapPair>,
            #[rust_sitter::leaf(text = "}")] (),
        ),

        // 4. Field Access (Dot Notation)
        // user.name OR ptr.name (Auto-Deref)
        /// Field access: `obj.field`.
        #[rust_sitter::prec_left(6)] // High precedence
        FieldAccess(
            Box<Expression>,
            #[rust_sitter::leaf(text = ".")] (),
            FieldNameVal, // Field Name
        ),
        /// Indexed/key access command: `collection at key`.
        // 4. Access Command: list at index
        #[rust_sitter::prec_left(5)] // High precedence
        At(
            Box<Expression>, // The Collection
            #[rust_sitter::leaf(text = "at")] (),
            Box<Expression>, // The Index/Key
        ),

        /// List append command: `list push value`.
        // 5. Modification Command: list push value
        #[rust_sitter::prec_left(5)]
        Push(
            Box<Expression>, // The List
            #[rust_sitter::leaf(text = "push")] (),
            Box<Expression>, // The Value
        ),
        /// Boolean literal expression.
        // 2. New Literals
        #[rust_sitter::prec_left(1)]
        BoolLit(BoolVal),

        /// Numeric literal expression.
        #[rust_sitter::prec_left(1)]
        Number(NumberVal),

        /// String literal expression.
        #[rust_sitter::prec_left(1)]
        StringLit(StringVal),

        /// Variable reference expression.
        #[rust_sitter::prec_left(1)]
        // 5. Variable Reference
        Variable(VariableVal),

        /// Move expression: transfers ownership-like state from mutable variable.
        // 6. Move Expression: move x
        #[rust_sitter::prec_right(10)]
        MoveExpr(#[rust_sitter::leaf(text = "move")] (), VariableVal),

        /// Error value reference (capitalized error type name).
        // 7. Error Reference (Capitalized)
        // Treated as a Value expression looking up an Error Type
        #[rust_sitter::prec_left(1)]
        ErrorRef(StructNameVal),

        /// Pointer initializer expression: `adr <type>`.
        #[rust_sitter::prec_left(1)]
        AdrInit(#[rust_sitter::leaf(text = "adr")] (), KiroType),

        /// Pipe initializer expression: `pipe <type>` or bounded `pipe <type> <capacity>`.
        #[rust_sitter::prec_left(1)]
        PipeInit(#[rust_sitter::leaf(text = "pipe")] (), KiroType, Option<NumberVal>),

        /// Receive from pipe: `take ch`.
        // 4. Take: take <channel>
        // Example: var x = take p
        #[rust_sitter::prec_right(4)]
        Take(#[rust_sitter::leaf(text = "take")] (), Box<Expression>),

        /// Length query: `len value`.
        // 5. Len: len <collection>
        #[rust_sitter::prec_right(4)]
        Len(#[rust_sitter::leaf(text = "len")] (), Box<Expression>),

        /// Create pointer/reference: `ref value`.
        // 3. Pointer Logic
        // ref x
        #[rust_sitter::prec_right(4)] // Right-associative
        Ref(#[rust_sitter::leaf(text = "ref")] (), Box<Expression>),

        /// Dereference pointer value: `deref ptr`.
        // deref x
        #[rust_sitter::prec_right(4)]
        Deref(#[rust_sitter::leaf(text = "deref")] (), Box<Expression>),
        /// Function call expression.
        #[rust_sitter::prec_left(3)] // High precedence
        Call(
            Box<Expression>, // The function name (usually a Variable)
            #[rust_sitter::leaf(text = "(")] (),
            #[rust_sitter::delimited(
                #[rust_sitter::leaf(text = ",")] ()
            )]
            Vec<Expression>, // Arguments
            #[rust_sitter::leaf(text = ")")] (),
        ),

        /// Async spawn call expression: `run fn_call(...)`.
        // 5. Async "Run" Call
        // Syntax: run foo(1, 2)
        #[rust_sitter::prec_left(2)]
        RunCall(
            #[rust_sitter::leaf(text = "run")] (),
            Box<Expression>, // Should be a Call expression
        ),
        #[rust_sitter::prec_left(2)]
        Mul(
            Box<Expression>,
            #[rust_sitter::leaf(text = "*")] (),
            Box<Expression>,
        ),
        #[rust_sitter::prec_left(2)]
        Div(
            Box<Expression>,
            #[rust_sitter::leaf(text = "/")] (),
            Box<Expression>,
        ),
        // Level 1: Addition & Subtraction (Happens Last)
        #[rust_sitter::prec_left(1)]
        Add(
            Box<Expression>,
            #[rust_sitter::leaf(text = "+")] (),
            Box<Expression>,
        ),
        #[rust_sitter::prec_left(1)]
        Sub(
            Box<Expression>,
            #[rust_sitter::leaf(text = "-")] (),
            Box<Expression>,
        ),
        #[rust_sitter::prec_left(0)]
        Eq(
            Box<Expression>,
            #[rust_sitter::leaf(text = "==")] (),
            Box<Expression>,
        ),
        #[rust_sitter::prec_left(0)]
        Neq(
            Box<Expression>,
            #[rust_sitter::leaf(text = "!=")] (),
            Box<Expression>,
        ),
        #[rust_sitter::prec_left(0)]
        Gt(
            Box<Expression>,
            #[rust_sitter::leaf(text = ">")] (),
            Box<Expression>,
        ),
        #[rust_sitter::prec_left(0)]
        Lt(
            Box<Expression>,
            #[rust_sitter::leaf(text = "<")] (),
            Box<Expression>,
        ),
        #[rust_sitter::prec_left(0)]
        Geq(
            Box<Expression>,
            #[rust_sitter::leaf(text = ">=")] (),
            Box<Expression>,
        ),
        #[rust_sitter::prec_left(0)]
        Leq(
            Box<Expression>,
            #[rust_sitter::leaf(text = "<=")] (),
            Box<Expression>,
        ),
        #[rust_sitter::prec_left(0)] // Low priority
        Range(
            Box<Expression>,
            #[rust_sitter::leaf(text = "..")] (),
            Box<Expression>,
        ),
    }
    // 7. Documentation Comments (/// ...)
    #[derive(Debug, Clone)]
    pub struct DocComment {
        #[rust_sitter::leaf(pattern = r"///[^\n]*", transform = |s| s.to_string())]
        pub content: String,
    }

    #[rust_sitter::extra]
    #[allow(dead_code)]
    pub struct Whitespace {
        // Match whitespace OR comments starting with // but NOT ///
        // Since Rust Regex doesn't support lookahead, we rely on tree-sitter matching DocComment first if defined.
        // But Whitespace is 'extra', so it has high precedence globally to be skipped?
        // Actually, if we define DocComment as a regular rule used in FunctionDef, tree-sitter *should* prioritize it
        // over the extra if it appears in a valid position.
        // However, safely excluding /// from the // rule is better.
        // Match whitespace OR comments starting with // but NOT ///
        // Pattern: // followed by (Start of line OR anything that isn't /)
        // We use |// to match empty comments or EOF case, relying on Longest Match to prefer DocComment for ///
        #[rust_sitter::leaf(pattern = r"\s+|//[^/][^\n]*|//")]
        _whitespace: (),
    }
    #[derive(Debug, Clone)]
    pub struct Block {
        #[rust_sitter::leaf(text = "{")]
        _l: (),
        #[rust_sitter::repeat(non_empty = false)]
        pub statements: Vec<Statement>,
        #[rust_sitter::leaf(text = "}")]
        _r: (),
    }
    #[derive(Debug, Clone)]
    pub struct OffClause {
        #[rust_sitter::leaf(text = "off")]
        _off: (),
        pub body: Block,
    }
    #[derive(Debug, Clone)]
    pub struct ErrorDesc {
        #[rust_sitter::leaf(text = "=")]
        _eq: (),
        pub value: StringVal,
    }
    #[derive(Debug, Clone)]
    #[rust_sitter::prec_right(2)]
    pub struct ErrorClause {
        #[rust_sitter::leaf(text = "error")]
        _error: (),
        // Optional error type (e.g., NotFound). None = catch-all handler.
        pub error_type: Option<StructNameVal>,
        pub body: Block,
    }
    // Recursive linked-list pattern for multiple error clauses
    #[derive(Debug, Clone)]
    #[rust_sitter::prec_right(2)]
    pub struct ErrorClauseList {
        pub first: ErrorClause,
        // Recursive: rest of error clauses (Box to break recursion)
        pub rest: Option<Box<ErrorClauseList>>,
    }
    #[derive(Debug, Clone)]
    pub struct StepClause {
        #[rust_sitter::leaf(text = "per")]
        _per: (),
        pub value: Expression,
    }
    #[derive(Debug, Clone)]
    pub struct LoopFilter {
        #[rust_sitter::leaf(text = "on")]
        _on: (),
        #[rust_sitter::leaf(text = "(")]
        _l: (),
        pub condition: Expression,
        #[rust_sitter::leaf(text = ")")]
        _r: (),
    }
    #[derive(Debug, Clone)]
    pub enum AnnotatableItem {
        StructDef(StructDef),
        FunctionDef(FunctionDef),
        RustFnDecl(RustFnDecl),
    }

    #[derive(Debug, Clone)]
    pub struct StructDef {
        #[rust_sitter::leaf(text = "struct")]
        pub _struct: (),

        // Struct names must be Capitalized to distinguish from variables
        pub name: StructNameVal,

        #[rust_sitter::leaf(text = "{")]
        pub _l: (),

        #[rust_sitter::repeat(non_empty = false)]
        pub fields: Vec<FieldDef>,

        #[rust_sitter::leaf(text = "}")]
        pub _r: (),
    }

    #[derive(Debug, Clone)]
    pub struct FunctionDef {
        #[rust_sitter::leaf(text = "pure")]
        pub pure_kw: Option<()>, // Optional "pure" keyword

        #[rust_sitter::leaf(text = "fn")]
        pub _fn: (),

        #[rust_sitter::leaf(pattern = r"[a-z_]+", transform = |s| s.to_string())]
        pub name: String,

        #[rust_sitter::leaf(text = "(")]
        pub _l: (),
        #[rust_sitter::delimited(
            #[rust_sitter::leaf(text = ",")] ()
        )]
        pub params: Vec<FuncParam>,
        #[rust_sitter::leaf(text = ")")]
        pub _r: (),

        #[rust_sitter::leaf(text = "->")]
        pub _arrow: Option<()>,
        pub return_type: Option<KiroType>,
        #[rust_sitter::leaf(text = "!")]
        pub can_error: Option<()>,

        pub body: Block, // Required body for normal functions
    }

    #[derive(Debug, Clone)]
    pub struct RustFnDecl {
        #[rust_sitter::leaf(text = "rust")]
        pub _rust_kw: (),

        #[rust_sitter::leaf(text = "fn")]
        pub _fn: (),

        #[rust_sitter::leaf(pattern = r"[a-z_]+", transform = |s| s.to_string())]
        pub name: String,

        #[rust_sitter::leaf(text = "(")]
        pub _l: (),
        #[rust_sitter::delimited(
            #[rust_sitter::leaf(text = ",")] ()
        )]
        pub params: Vec<FuncParam>,
        #[rust_sitter::leaf(text = ")")]
        pub _r: (),

        #[rust_sitter::leaf(text = "->")]
        pub _arrow: (), // REQUIRED
        pub return_type: KiroType, // REQUIRED
        #[rust_sitter::leaf(text = "!")]
        pub can_error: Option<()>,
        // No body - this is an external function
    }
}
pub use grammar::*;

pub fn parse(input: &str) -> Result<Program, Vec<rust_sitter::errors::ParseError>> {
    let program = grammar::parse(input)?;
    Ok(normalize_pipe_capacity(program))
}

fn normalize_pipe_capacity(program: Program) -> Program {
    let mut out: Vec<Statement> = Vec::new();
    let mut i = 0usize;

    while i < program.statements.len() {
        let stmt = program.statements[i].clone();
        let next_num = if i + 1 < program.statements.len() {
            match &program.statements[i + 1] {
                Statement::ExprStmt(Expression::Number(n)) => Some(n.clone()),
                _ => None,
            }
        } else {
            None
        };

        if let Some(cap) = next_num {
            match stmt {
                Statement::VarDecl {
                    _var,
                    ident,
                    _eq,
                    value: Expression::PipeInit(pipe_kw, pipe_ty, None),
                } => {
                    out.push(Statement::VarDecl {
                        _var,
                        ident,
                        _eq,
                        value: Expression::PipeInit(pipe_kw, pipe_ty, Some(cap)),
                    });
                    i += 2;
                    continue;
                }
                Statement::AssignStmt {
                    lhs,
                    _eq,
                    rhs: Expression::PipeInit(pipe_kw, pipe_ty, None),
                } => {
                    out.push(Statement::AssignStmt {
                        lhs,
                        _eq,
                        rhs: Expression::PipeInit(pipe_kw, pipe_ty, Some(cap)),
                    });
                    i += 2;
                    continue;
                }
                Statement::ExprStmt(Expression::PipeInit(pipe_kw, pipe_ty, None)) => {
                    out.push(Statement::ExprStmt(Expression::PipeInit(
                        pipe_kw,
                        pipe_ty,
                        Some(cap),
                    )));
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }

        out.push(stmt);
        i += 1;
    }

    Program { statements: out }
}
