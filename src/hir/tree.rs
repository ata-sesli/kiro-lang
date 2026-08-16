use std::collections::BTreeMap;

use super::{
    Effects, ErrorId, FieldId, FunctionId, HandleId, HostFunctionId, LocalId, ModuleId, Signature,
    SourceAnchor, StructId, TypeId, TypeTable,
};

#[derive(Debug, Clone)]
pub struct HirProgram {
    pub types: TypeTable,
    pub modules: Vec<HirModule>,
    pub symbols: HirSymbols,
}

impl HirProgram {
    pub fn module(&self, name: &str) -> Option<&HirModule> {
        self.modules.iter().find(|module| module.name == name)
    }
}

#[derive(Debug, Clone)]
pub struct HirModule {
    pub id: ModuleId,
    pub name: String,
    pub statements: Vec<HirStmt>,
    pub functions: Vec<HirFunction>,
    pub host_functions: Vec<HirHostFunction>,
    pub structs: Vec<HirStruct>,
    function_indices: BTreeMap<String, usize>,
    host_function_indices: BTreeMap<String, usize>,
}

impl HirModule {
    pub(crate) fn empty(name: String) -> Self {
        Self::new(
            ModuleId::new(0),
            name,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    pub(crate) fn new(
        id: ModuleId,
        name: String,
        statements: Vec<HirStmt>,
        functions: Vec<HirFunction>,
        host_functions: Vec<HirHostFunction>,
        structs: Vec<HirStruct>,
    ) -> Self {
        let function_indices = functions
            .iter()
            .enumerate()
            .map(|(index, function)| (function.name.clone(), index))
            .collect();
        let host_function_indices = host_functions
            .iter()
            .enumerate()
            .map(|(index, function)| (function.name.clone(), index))
            .collect();
        Self {
            id,
            name,
            statements,
            functions,
            host_functions,
            structs,
            function_indices,
            host_function_indices,
        }
    }

    pub fn function(&self, name: &str) -> Option<&HirFunction> {
        self.function_indices
            .get(name)
            .and_then(|index| self.functions.get(*index))
    }

    pub fn host_function(&self, name: &str) -> Option<&HirHostFunction> {
        self.host_function_indices
            .get(name)
            .and_then(|index| self.host_functions.get(*index))
    }
}

#[derive(Debug, Clone, Default)]
pub struct HirSymbols {
    pub modules: Vec<String>,
    pub functions: Vec<String>,
    pub host_functions: Vec<String>,
    pub structs: Vec<String>,
    pub handles: Vec<String>,
    pub errors: Vec<String>,
    pub fields: Vec<String>,
    pub locals: Vec<LocalSymbol>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSymbol {
    pub owner: Option<FunctionId>,
    pub id: LocalId,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct HirFunction {
    pub id: FunctionId,
    pub name: String,
    pub params: Vec<HirParam>,
    pub signature: Signature,
    pub body: Vec<HirStmt>,
    pub anchor: SourceAnchor,
}

#[derive(Debug, Clone)]
pub struct HirHostFunction {
    pub id: HostFunctionId,
    pub name: String,
    pub params: Vec<HirParam>,
    pub signature: Signature,
    pub anchor: SourceAnchor,
}

#[derive(Debug, Clone)]
pub struct HirParam {
    pub local: LocalId,
    pub ty: TypeId,
    pub anchor: SourceAnchor,
}

#[derive(Debug, Clone)]
pub struct HirStruct {
    pub id: StructId,
    pub fields: Vec<HirStructField>,
    pub anchor: SourceAnchor,
}

#[derive(Debug, Clone)]
pub struct HirStructField {
    pub id: FieldId,
    pub ty: TypeId,
    pub anchor: SourceAnchor,
}

#[derive(Debug, Clone)]
pub struct HirStmt {
    pub kind: HirStmtKind,
    pub anchor: SourceAnchor,
}

#[derive(Debug, Clone)]
pub enum HirStmtKind {
    ErrorDef {
        id: ErrorId,
        description: Option<String>,
    },
    StructDef(StructId),
    HandleDef(HandleId),
    VarDecl {
        local: LocalId,
        value: HirExpr,
    },
    Assign {
        target: HirExpr,
        value: HirExpr,
    },
    On {
        condition: HirExpr,
        body: Vec<HirStmt>,
        else_body: Option<Vec<HirStmt>>,
        error_clauses: Vec<HirErrorClause>,
    },
    LoopOn {
        condition: HirExpr,
        body: Vec<HirStmt>,
    },
    LoopIter {
        iterator: LocalId,
        iterable: HirExpr,
        step: Option<HirExpr>,
        filter: Option<HirExpr>,
        body: Vec<HirStmt>,
        else_body: Option<Vec<HirStmt>>,
    },
    FunctionDef(FunctionId),
    HostFunctionDecl(HostFunctionId),
    Give {
        channel: HirExpr,
        value: HirExpr,
    },
    Close(HirExpr),
    Return(Option<HirExpr>),
    Break,
    Continue,
    Rest,
    Check {
        condition: HirExpr,
        message: Option<String>,
    },
    Import(ModuleId),
    Expr(HirExpr),
}

#[derive(Debug, Clone)]
pub struct HirErrorClause {
    pub error: Option<ErrorId>,
    pub body: Vec<HirStmt>,
}

#[derive(Debug, Clone)]
pub struct HirExpr {
    pub kind: HirExprKind,
    pub ty: TypeId,
    pub anchor: SourceAnchor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirCollectionOp {
    ListJoin,
    ListSlice,
    ListReverse,
    MapHas,
    MapSet,
    MapDelete,
}

#[derive(Debug, Clone)]
pub enum HirExprKind {
    StructInit {
        structure: StructId,
        fields: Vec<HirFieldInit>,
    },
    ListInit(Vec<HirExpr>),
    MapInit(Vec<HirMapPair>),
    CollectionCall {
        op: HirCollectionOp,
        args: Vec<HirExpr>,
    },
    FieldAccess {
        target: Box<HirExpr>,
        field: FieldId,
    },
    At {
        collection: Box<HirExpr>,
        key: Box<HirExpr>,
    },
    Push {
        collection: Box<HirExpr>,
        value: Box<HirExpr>,
    },
    Bool(bool),
    Number(f64),
    String(String),
    Local(LocalId),
    Module(ModuleId),
    Function(FunctionId),
    HostFunction(HostFunctionId),
    Move(LocalId),
    Error(ErrorId),
    AddressInit,
    PipeInit {
        capacity: Option<usize>,
    },
    Take(Box<HirExpr>),
    Len(Box<HirExpr>),
    Ref(Box<HirExpr>),
    Deref(Box<HirExpr>),
    Call {
        kind: HirCallKind,
        args: Vec<HirExpr>,
    },
    RunCall(Box<HirExpr>),
    Binary {
        op: HirBinaryOp,
        lhs: Box<HirExpr>,
        rhs: Box<HirExpr>,
    },
}

#[derive(Debug, Clone)]
pub struct HirFieldInit {
    pub field: FieldId,
    pub value: HirExpr,
}

#[derive(Debug, Clone)]
pub struct HirMapPair {
    pub key: HirExpr,
    pub value: HirExpr,
}

#[derive(Debug, Clone)]
pub enum HirCallKind {
    Direct(FunctionId),
    Host(HostFunctionId),
    Indirect(Box<HirExpr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirBinaryOp {
    AddNum,
    ConcatString,
    SubNum,
    MulNum,
    DivNum,
    EqNum,
    EqString,
    EqBool,
    NeNum,
    NeString,
    NeBool,
    GtNum,
    LtNum,
    GeNum,
    LeNum,
    RangeNum,
}

impl HirFunction {
    pub fn effects(&self) -> Effects {
        self.signature.effects()
    }
}
