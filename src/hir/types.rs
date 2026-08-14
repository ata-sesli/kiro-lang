use std::collections::HashMap;
use std::fmt;

use super::{ErrorId, HandleId, StructId, TypeId};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SemType {
    Unknown,
    Void,
    Bool,
    Num,
    Str,
    Range,
    Address(TypeId),
    Pipe(TypeId),
    List(TypeId),
    Map(TypeId, TypeId),
    Function {
        params: Box<[TypeId]>,
        return_type: TypeId,
    },
    Struct(StructId),
    Handle(HandleId),
    Error(ErrorId),
}

impl TypeId {
    pub const UNKNOWN: Self = Self::new(0);
    pub const VOID: Self = Self::new(1);
    pub const BOOL: Self = Self::new(2);
    pub const NUM: Self = Self::new(3);
    pub const STR: Self = Self::new(4);
}

#[derive(Clone)]
pub struct TypeTable {
    types: Vec<SemType>,
    ids: HashMap<SemType, TypeId>,
}

impl TypeTable {
    pub fn new() -> Self {
        let types = vec![
            SemType::Unknown,
            SemType::Void,
            SemType::Bool,
            SemType::Num,
            SemType::Str,
        ];
        let ids = types
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, ty)| {
                let id = TypeId::try_from(index).expect("primitive type IDs must fit u32");
                (ty, id)
            })
            .collect();
        Self { types, ids }
    }

    pub fn intern(&mut self, ty: SemType) -> TypeId {
        if let Some(id) = self.ids.get(&ty) {
            return *id;
        }

        let id = TypeId::try_from(self.types.len()).expect("type table exceeds u32 ID capacity");
        self.types.push(ty.clone());
        self.ids.insert(ty, id);
        id
    }

    pub fn get(&self, id: TypeId) -> Option<&SemType> {
        let index = usize::try_from(id).ok()?;
        self.types.get(index)
    }

    pub fn len(&self) -> usize {
        self.types.len()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }
}

impl Default for TypeTable {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for TypeTable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("TypeTable")
            .field(&self.types)
            .finish()
    }
}
