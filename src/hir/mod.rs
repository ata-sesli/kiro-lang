mod effects;
mod ids;
mod tree;
mod types;

use std::fmt;
use std::ops::Range;

pub use effects::Effects;
pub use ids::{
    ErrorId, FieldId, FunctionId, HandleId, HostFunctionId, IdOverflow, LocalId, ModuleId,
    SourceId, StructId, TypeId,
};
pub use tree::{
    HirBinaryOp, HirCallKind, HirErrorClause, HirExpr, HirExprKind, HirFieldInit, HirFunction,
    HirHostFunction, HirMapPair, HirModule, HirParam, HirProgram, HirStmt, HirStmtKind, HirStruct,
    HirStructField, HirSymbols, LocalSymbol,
};
pub use types::{SemType, TypeTable};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    params: Box<[TypeId]>,
    return_type: TypeId,
    effects: Effects,
}

impl Signature {
    pub fn new(params: impl Into<Box<[TypeId]>>, return_type: TypeId, effects: Effects) -> Self {
        Self {
            params: params.into(),
            return_type,
            effects,
        }
    }

    pub fn params(&self) -> &[TypeId] {
        &self.params
    }

    pub const fn return_type(&self) -> TypeId {
        self.return_type
    }

    pub const fn effects(&self) -> Effects {
        self.effects
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceAnchor {
    source: SourceId,
    start: u32,
    end: u32,
}

impl SourceAnchor {
    pub fn try_from_offsets(
        source: SourceId,
        start: usize,
        end: usize,
    ) -> Result<Self, SourceAnchorError> {
        if start > end {
            return Err(SourceAnchorError::ReversedRange { start, end });
        }
        let start = u32::try_from(start)
            .map_err(|_| SourceAnchorError::OffsetOutOfRange { offset: start })?;
        let end =
            u32::try_from(end).map_err(|_| SourceAnchorError::OffsetOutOfRange { offset: end })?;
        Ok(Self { source, start, end })
    }

    pub const fn source(self) -> SourceId {
        self.source
    }

    pub const fn start(self) -> u32 {
        self.start
    }

    pub const fn end(self) -> u32 {
        self.end
    }

    pub const fn range(self) -> Range<u32> {
        self.start..self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceAnchorError {
    ReversedRange { start: usize, end: usize },
    OffsetOutOfRange { offset: usize },
}

impl fmt::Display for SourceAnchorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReversedRange { start, end } => {
                write!(
                    formatter,
                    "source range starts at {start} after ending at {end}"
                )
            }
            Self::OffsetOutOfRange { offset } => {
                write!(formatter, "source offset {offset} exceeds the u32 range")
            }
        }
    }
}

impl std::error::Error for SourceAnchorError {}
