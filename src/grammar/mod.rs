#[rust_sitter::grammar("kiro")]
pub mod grammar {
    use rust_sitter::Spanned;

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
    impl std::ops::Deref for NumberVal {
        type Target = str;

        fn deref(&self) -> &Self::Target {
            &self.value
        }
    }
    impl std::fmt::Display for NumberVal {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.value.fmt(f)
        }
    }
    #[derive(Debug, Clone)]
    pub struct VariableVal {
        #[rust_sitter::leaf(pattern = r"[a-z_][a-zA-Z0-9_]*", transform = |s| s.to_string())]
        pub value: String,
    }
    impl VariableVal {
        pub fn name(&self) -> &str {
            &self.value
        }
    }
    impl std::ops::Deref for VariableVal {
        type Target = str;

        fn deref(&self) -> &Self::Target {
            &self.value
        }
    }
    impl std::fmt::Display for VariableVal {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.value.fmt(f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct FunctionNameVal {
        #[rust_sitter::leaf(pattern = r"[a-z_]+", transform = |s| s.to_string())]
        pub value: String,
    }
    impl FunctionNameVal {
        pub fn name(&self) -> &str {
            &self.value
        }
    }
    impl std::ops::Deref for FunctionNameVal {
        type Target = str;

        fn deref(&self) -> &Self::Target {
            &self.value
        }
    }
    impl std::fmt::Display for FunctionNameVal {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.value.fmt(f)
        }
    }

    // 2. Wrapper for String Literals ("hello")
    #[derive(Debug, Clone)]
    pub struct StringVal {
        #[rust_sitter::leaf(pattern = r#""([^"\\]|\\.)*""#, transform = |s| s.to_string())]
        pub value: String,
    }
    impl std::ops::Deref for StringVal {
        type Target = str;

        fn deref(&self) -> &Self::Target {
            &self.value
        }
    }
    impl std::fmt::Display for StringVal {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.value.fmt(f)
        }
    }

    #[derive(Debug, Clone)]
    pub struct CheckMessage {
        #[rust_sitter::leaf(text = ",")]
        _comma: (),
        pub value: StringVal,
    }
    // 3. For Struct Names (Capitalized: "User")
    #[derive(Debug, Clone)]
    pub struct StructNameVal {
        #[rust_sitter::leaf(pattern = r"[A-Z][a-zA-Z0-9_]*", transform = |s| s.to_string())]
        pub value: String,
    }
    impl StructNameVal {
        pub fn name(&self) -> &str {
            &self.value
        }
    }
    impl std::ops::Deref for StructNameVal {
        type Target = str;

        fn deref(&self) -> &Self::Target {
            &self.value
        }
    }
    impl std::fmt::Display for StructNameVal {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.value.fmt(f)
        }
    }

    // 4. For Field Names (Lowercase: "age")
    #[derive(Debug, Clone)]
    pub struct FieldNameVal {
        #[rust_sitter::leaf(pattern = r"[a-z_]+", transform = |s| s.to_string())]
        pub value: String,
    }
    impl FieldNameVal {
        pub fn name(&self) -> &str {
            &self.value
        }
    }
    impl std::ops::Deref for FieldNameVal {
        type Target = str;

        fn deref(&self) -> &Self::Target {
            &self.value
        }
    }
    impl std::fmt::Display for FieldNameVal {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.value.fmt(f)
        }
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
        pub name: Spanned<VariableVal>,
        #[rust_sitter::leaf(text = ":")]
        _colon: (),
        pub command_type: KiroType,
    }
    // A single field in a struct definition: "name: str"
    #[derive(Debug, Clone)]
    pub struct FieldDef {
        pub name: Spanned<FieldNameVal>,
        #[rust_sitter::leaf(text = ":")]
        _colon: (),
        pub field_type: KiroType,
    }

    // A single field assignment in initialization: "name: 'Kiro'"
    #[derive(Debug, Clone)]
    pub struct FieldInit {
        pub name: Spanned<FieldNameVal>,
        #[rust_sitter::leaf(text = ":")]
        _colon: (),
        pub value: Expression,
    }

    #[derive(Debug, Clone)]
    pub enum Statement {
        // ... (Keep existing Statements) ...
        /// Opaque host-owned handle declaration: `handle File`.
        HandleDef(HandleDef),

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
            _error: Spanned<()>,
            name: Spanned<StructNameVal>,
            description: Option<ErrorDesc>,
        },
        /// Mutable variable declaration: `var x = expr`.
        // 1. Variable Declaration: var x = 10
        VarDecl {
            #[rust_sitter::leaf(text = "var")]
            _var: Spanned<()>,
            ident: Spanned<VariableVal>,
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
            _loop: Spanned<()>,
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
            _loop: Spanned<()>,
            iterator: Spanned<VariableVal>,
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
            #[rust_sitter::leaf(text = "give")] Spanned<()>,
            Expression, // Channel
            Expression, // Value
        ),

        /// Close a pipe sender: `close ch`.
        // 2. Close: close <channel>
        Close(
            #[rust_sitter::leaf(text = "close")] Spanned<()>,
            Expression, // Channel
        ),
        /// Return from current function.
        // 3. Return Statement
        #[rust_sitter::prec_right(1)]
        Return(
            #[rust_sitter::leaf(text = "return")] Spanned<()>,
            Option<Expression>,
        ),
        /// Break from current loop.
        // 4. Break Statement
        Break(#[rust_sitter::leaf(text = "break")] Spanned<()>),
        /// Continue to next loop iteration.
        // 5. Continue Statement
        Continue(#[rust_sitter::leaf(text = "continue")] Spanned<()>),
        /// Cooperative scheduler rest point: `rest`.
        Rest(#[rust_sitter::leaf(text = "rest")] Spanned<()>),
        /// Runtime guard: `check condition` or `check condition, "message"`.
        Check(
            #[rust_sitter::leaf(text = "check")] Spanned<()>,
            Expression,
            Option<CheckMessage>,
        ),

        /// Import module by name: `import math`.
        // 6. Import Statement
        Import {
            #[rust_sitter::leaf(text = "import")]
            _import: Spanned<()>,
            module_name: Spanned<VariableVal>,
        },

        /// Expression as statement.
        ExprStmt(Expression),

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
            Spanned<StructNameVal>, // Struct Name
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
            #[rust_sitter::leaf(text = "list")] Spanned<()>,
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
            #[rust_sitter::leaf(text = "map")] Spanned<()>,
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
            #[rust_sitter::leaf(text = ".")] Spanned<()>,
            Spanned<FieldNameVal>, // Field Name
        ),
        /// Indexed/key access command: `collection at key`.
        // 4. Access Command: list at index
        #[rust_sitter::prec_left(5)] // High precedence
        At(
            Box<Expression>, // The Collection
            #[rust_sitter::leaf(text = "at")] Spanned<()>,
            Box<Expression>, // The Index/Key
        ),

        /// List append command: `list push value`.
        // 5. Modification Command: list push value
        #[rust_sitter::prec_left(5)]
        Push(
            Box<Expression>, // The List
            #[rust_sitter::leaf(text = "push")] Spanned<()>,
            Box<Expression>, // The Value
        ),
        /// Boolean literal expression.
        // 2. New Literals
        #[rust_sitter::prec_left(1)]
        BoolLit(Spanned<BoolVal>),

        /// Numeric literal expression.
        #[rust_sitter::prec_left(1)]
        Number(Spanned<NumberVal>),

        /// String literal expression.
        #[rust_sitter::prec_left(1)]
        StringLit(Spanned<StringVal>),

        /// Variable reference expression.
        #[rust_sitter::prec_left(1)]
        // 5. Variable Reference
        Variable(Spanned<VariableVal>),

        /// Move expression: transfers ownership-like state from mutable variable.
        // 6. Move Expression: move x
        #[rust_sitter::prec_right(10)]
        MoveExpr(
            #[rust_sitter::leaf(text = "move")] Spanned<()>,
            Spanned<VariableVal>,
        ),

        /// Error value reference (capitalized error type name).
        // 7. Error Reference (Capitalized)
        // Treated as a Value expression looking up an Error Type
        #[rust_sitter::prec_left(1)]
        ErrorRef(Spanned<StructNameVal>),

        /// Pointer initializer expression: `adr <type>`.
        #[rust_sitter::prec_left(1)]
        AdrInit(#[rust_sitter::leaf(text = "adr")] Spanned<()>, KiroType),

        /// Pipe initializer expression: `pipe <type>` or bounded `pipe <type> <capacity>`.
        #[rust_sitter::prec_left(1)]
        PipeInit(
            #[rust_sitter::leaf(text = "pipe")] Spanned<()>,
            KiroType,
            Option<NumberVal>,
        ),

        /// Receive from pipe: `take ch`.
        // 4. Take: take <channel>
        // Example: var x = take p
        #[rust_sitter::prec_right(4)]
        Take(
            #[rust_sitter::leaf(text = "take")] Spanned<()>,
            Box<Expression>,
        ),

        /// Length query: `len value`.
        // 5. Len: len <collection>
        #[rust_sitter::prec_right(4)]
        Len(
            #[rust_sitter::leaf(text = "len")] Spanned<()>,
            Box<Expression>,
        ),

        /// Create pointer/reference: `ref value`.
        // 3. Pointer Logic
        // ref x
        #[rust_sitter::prec_right(4)] // Right-associative
        Ref(
            #[rust_sitter::leaf(text = "ref")] Spanned<()>,
            Box<Expression>,
        ),

        /// Dereference pointer value: `deref ptr`.
        // deref x
        #[rust_sitter::prec_right(4)]
        Deref(
            #[rust_sitter::leaf(text = "deref")] Spanned<()>,
            Box<Expression>,
        ),
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
            #[rust_sitter::leaf(text = "run")] Spanned<()>,
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
        HandleDef(HandleDef),
        StructDef(StructDef),
        FunctionDef(FunctionDef),
        RustFnDecl(RustFnDecl),
    }

    #[derive(Debug, Clone)]
    pub struct HandleDef {
        #[rust_sitter::leaf(text = "handle")]
        pub _handle: Spanned<()>,

        pub name: Spanned<StructNameVal>,
    }

    #[derive(Debug, Clone)]
    pub struct StructDef {
        #[rust_sitter::leaf(text = "struct")]
        pub _struct: Spanned<()>,

        // Struct names must be Capitalized to distinguish from variables
        pub name: Spanned<StructNameVal>,

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

        pub name: Spanned<FunctionNameVal>,

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
        #[rust_sitter::leaf(text = "pure")]
        pub pure_kw: Option<()>,

        #[rust_sitter::leaf(text = "rust")]
        pub _rust_kw: Spanned<()>,

        #[rust_sitter::leaf(text = "fn")]
        pub _fn: Spanned<()>,

        pub name: Spanned<FunctionNameVal>,

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

pub type AstSpan = (usize, usize);

pub fn span_start<T>(value: &rust_sitter::Spanned<T>) -> usize {
    value.span.0
}

pub fn span_end<T>(value: &rust_sitter::Spanned<T>) -> usize {
    value.span.1
}

pub fn span_len(span: AstSpan) -> usize {
    span.1.saturating_sub(span.0).max(1)
}

pub fn variable_name(value: &rust_sitter::Spanned<VariableVal>) -> &str {
    value.name()
}

pub fn variable_span(value: &rust_sitter::Spanned<VariableVal>) -> AstSpan {
    value.span
}

pub fn param_name(value: &FuncParam) -> &str {
    variable_name(&value.name)
}

pub fn param_name_span(value: &FuncParam) -> AstSpan {
    variable_span(&value.name)
}

pub fn function_name(value: &rust_sitter::Spanned<FunctionNameVal>) -> &str {
    value.name()
}

pub fn function_span(value: &rust_sitter::Spanned<FunctionNameVal>) -> AstSpan {
    value.span
}

pub fn rust_fn_decl_span(value: &RustFnDecl) -> AstSpan {
    (value._rust_kw.span.0, value.name.span.1)
}

pub fn field_name(value: &rust_sitter::Spanned<FieldNameVal>) -> &str {
    value.name()
}

pub fn field_span(value: &rust_sitter::Spanned<FieldNameVal>) -> AstSpan {
    value.span
}

pub fn field_def_name(value: &FieldDef) -> &str {
    field_name(&value.name)
}

pub fn field_def_span(value: &FieldDef) -> AstSpan {
    field_span(&value.name)
}

pub fn struct_name(value: &rust_sitter::Spanned<StructNameVal>) -> &str {
    value.name()
}

pub fn struct_span(value: &rust_sitter::Spanned<StructNameVal>) -> AstSpan {
    value.span
}

pub fn struct_def_name(value: &StructDef) -> &str {
    struct_name(&value.name)
}

pub fn struct_def_span(value: &StructDef) -> AstSpan {
    struct_span(&value.name)
}

pub fn handle_name(value: &HandleDef) -> &str {
    struct_name(&value.name)
}

pub fn handle_span(value: &HandleDef) -> AstSpan {
    struct_span(&value.name)
}

pub fn error_name(value: &Statement) -> Option<&str> {
    match value {
        Statement::ErrorDef { name, .. } => Some(struct_name(name)),
        _ => None,
    }
}

pub fn error_name_span(value: &Statement) -> Option<AstSpan> {
    match value {
        Statement::ErrorDef { name, .. } => Some(struct_span(name)),
        _ => None,
    }
}

pub fn var_decl_name(value: &Statement) -> Option<&str> {
    match value {
        Statement::VarDecl { ident, .. } => Some(variable_name(ident)),
        _ => None,
    }
}

pub fn var_decl_span(value: &Statement) -> Option<AstSpan> {
    match value {
        Statement::VarDecl { ident, .. } => Some(variable_span(ident)),
        _ => None,
    }
}

pub fn loop_iterator_name(value: &Statement) -> Option<&str> {
    match value {
        Statement::LoopIter { iterator, .. } => Some(variable_name(iterator)),
        _ => None,
    }
}

pub fn loop_iterator_span(value: &Statement) -> Option<AstSpan> {
    match value {
        Statement::LoopIter { iterator, .. } => Some(variable_span(iterator)),
        _ => None,
    }
}

pub fn import_name(value: &Statement) -> Option<&str> {
    match value {
        Statement::Import { module_name, .. } => Some(variable_name(module_name)),
        _ => None,
    }
}

pub fn import_span(value: &Statement) -> Option<AstSpan> {
    match value {
        Statement::Import { module_name, .. } => Some(variable_span(module_name)),
        _ => None,
    }
}

pub fn token_span<T>(value: &rust_sitter::Spanned<T>) -> AstSpan {
    value.span
}

pub fn stmt_span(stmt: &Statement) -> Option<AstSpan> {
    match stmt {
        Statement::HandleDef(def) => Some(handle_span(def)),
        Statement::StructDef(def) => Some(struct_def_span(def)),
        Statement::ErrorDef { name, .. } => Some(struct_span(name)),
        Statement::VarDecl { ident, .. } => Some(variable_span(ident)),
        Statement::AssignStmt { lhs, rhs, .. } => merge_expr_spans(lhs, rhs),
        Statement::On { condition, .. } => expr_span(condition),
        Statement::LoopOn {
            _loop, condition, ..
        } => {
            let end = expr_span(condition)
                .map(|span| span.1)
                .unwrap_or(_loop.span.1);
            Some((_loop.span.0, end))
        }
        Statement::LoopIter { _loop, body, .. } => {
            let end = body
                .statements
                .last()
                .and_then(stmt_span)
                .map(|span| span.1)
                .unwrap_or(_loop.span.1);
            Some((_loop.span.0, end))
        }
        Statement::FunctionDef(def) => Some(function_span(&def.name)),
        Statement::RustFnDecl(def) => Some(rust_fn_decl_span(def)),
        Statement::Give(keyword, ..)
        | Statement::Close(keyword, _)
        | Statement::Return(keyword, _)
        | Statement::Rest(keyword)
        | Statement::Check(keyword, _, _) => Some(keyword.span),
        Statement::Break(keyword) | Statement::Continue(keyword) => Some(keyword.span),
        Statement::Import { module_name, .. } => Some(variable_span(module_name)),
        Statement::ExprStmt(expr) => expr_span(expr),
        Statement::Documented { item, .. } => match item {
            AnnotatableItem::HandleDef(def) => Some(handle_span(def)),
            AnnotatableItem::StructDef(def) => Some(struct_def_span(def)),
            AnnotatableItem::FunctionDef(def) => Some(function_span(&def.name)),
            AnnotatableItem::RustFnDecl(def) => Some(rust_fn_decl_span(def)),
        },
    }
}

pub fn expr_span(expr: &Expression) -> Option<AstSpan> {
    match expr {
        Expression::Number(value) => Some(value.span),
        Expression::StringLit(value) => Some(value.span),
        Expression::BoolLit(value) => Some(value.span),
        Expression::Variable(value) => Some(variable_span(value)),
        Expression::ErrorRef(value) => Some(struct_span(value)),
        Expression::StructInit(name, _, _, _) => Some(struct_span(name)),
        Expression::MoveExpr(keyword, value) => Some((keyword.span.0, value.span.1)),
        Expression::FieldAccess(target, _, field) => {
            let start = expr_span(target).map(|span| span.0).unwrap_or(field.span.0);
            Some((start, field.span.1))
        }
        Expression::At(target, keyword, key) | Expression::Push(target, keyword, key) => {
            let start = expr_span(target)
                .map(|span| span.0)
                .unwrap_or(keyword.span.0);
            let end = expr_span(key).map(|span| span.1).unwrap_or(keyword.span.1);
            Some((start, end))
        }
        Expression::Take(keyword, target)
        | Expression::Len(keyword, target)
        | Expression::Ref(keyword, target)
        | Expression::Deref(keyword, target)
        | Expression::RunCall(keyword, target) => {
            let end = expr_span(target)
                .map(|span| span.1)
                .unwrap_or(keyword.span.1);
            Some((keyword.span.0, end))
        }
        Expression::ListInit(keyword, _, _, items, _) => {
            let end = items
                .last()
                .and_then(expr_span)
                .map(|span| span.1)
                .unwrap_or(keyword.span.1);
            Some((keyword.span.0, end))
        }
        Expression::MapInit(keyword, _, _, _, pairs, _) => {
            let end = pairs
                .last()
                .and_then(|pair| expr_span(&pair.value).or_else(|| expr_span(&pair.key)))
                .map(|span| span.1)
                .unwrap_or(keyword.span.1);
            Some((keyword.span.0, end))
        }
        Expression::AdrInit(keyword, _) | Expression::PipeInit(keyword, _, _) => Some(keyword.span),
        Expression::Call(func, _, _, _) => expr_span(func),
        Expression::Add(lhs, _, rhs)
        | Expression::Sub(lhs, _, rhs)
        | Expression::Mul(lhs, _, rhs)
        | Expression::Div(lhs, _, rhs)
        | Expression::Eq(lhs, _, rhs)
        | Expression::Neq(lhs, _, rhs)
        | Expression::Gt(lhs, _, rhs)
        | Expression::Lt(lhs, _, rhs)
        | Expression::Geq(lhs, _, rhs)
        | Expression::Leq(lhs, _, rhs)
        | Expression::Range(lhs, _, rhs) => {
            let start = expr_span(lhs)?;
            let end = expr_span(rhs)?;
            Some((start.0, end.1))
        }
    }
}

fn merge_expr_spans(lhs: &Expression, rhs: &Expression) -> Option<AstSpan> {
    let lhs = expr_span(lhs)?;
    let rhs = expr_span(rhs)?;
    Some((lhs.0, rhs.1))
}

pub fn call_target_span(func: &Expression) -> Option<AstSpan> {
    expr_span(func)
}

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
                Statement::ExprStmt(Expression::Number(n)) => Some(n.value.clone()),
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
