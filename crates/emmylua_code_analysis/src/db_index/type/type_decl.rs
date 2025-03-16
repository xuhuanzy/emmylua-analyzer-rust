use flagset::{flags, FlagSet};
use internment::ArcIntern;
use rowan::TextRange;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use smol_str::SmolStr;

use crate::{instantiate_type, DbIndex, FileId, TypeSubstitutor};

use super::LuaType;

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub enum LuaDeclTypeKind {
    Class,
    Enum,
    Alias,
}

flags! {
    pub enum LuaTypeAttribute: u8 {
        None,
        Key,
        Partial,
        Exact,
        Meta
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct LuaTypeDecl {
    simple_name: String,
    locations: Vec<LuaDeclLocation>,
    id: LuaTypeDeclId,
    extra: LuaTypeExtra,
}

impl LuaTypeDecl {
    pub fn new(
        file_id: FileId,
        range: TextRange,
        name: String,
        kind: LuaDeclTypeKind,
        attrib: FlagSet<LuaTypeAttribute>,
        id: LuaTypeDeclId,
    ) -> Self {
        Self {
            simple_name: name,
            locations: vec![LuaDeclLocation {
                file_id,
                range,
                attrib,
            }],
            id,
            extra: match kind {
                LuaDeclTypeKind::Enum => LuaTypeExtra::Enum { base: None },
                LuaDeclTypeKind::Class => LuaTypeExtra::Class,
                LuaDeclTypeKind::Alias => LuaTypeExtra::Alias { origin: None },
            },
        }
    }

    pub fn get_locations(&self) -> &[LuaDeclLocation] {
        &self.locations
    }

    pub fn get_mut_locations(&mut self) -> &mut Vec<LuaDeclLocation> {
        &mut self.locations
    }

    pub fn get_name(&self) -> &str {
        &self.simple_name
    }

    pub fn is_class(&self) -> bool {
        matches!(self.extra, LuaTypeExtra::Class)
    }

    pub fn is_enum(&self) -> bool {
        matches!(self.extra, LuaTypeExtra::Enum { .. })
    }

    pub fn is_alias(&self) -> bool {
        matches!(self.extra, LuaTypeExtra::Alias { .. })
    }

    pub fn is_exact(&self) -> bool {
        self.locations
            .iter()
            .any(|l| l.attrib.contains(LuaTypeAttribute::Exact))
    }

    pub fn is_partial(&self) -> bool {
        self.locations
            .iter()
            .any(|l| l.attrib.contains(LuaTypeAttribute::Partial))
    }

    pub fn is_enum_key(&self) -> bool {
        self.locations
            .iter()
            .any(|l| l.attrib.contains(LuaTypeAttribute::Key))
    }

    pub fn get_id(&self) -> LuaTypeDeclId {
        self.id.clone()
    }

    pub fn get_full_name(&self) -> &str {
        self.id.get_name()
    }

    pub fn get_namespace(&self) -> Option<&str> {
        self.id
            .get_name()
            .rfind('.')
            .map(|idx| &self.id.get_name()[..idx])
    }

    pub fn get_alias_origin(
        &self,
        db: &DbIndex,
        substitutor: Option<&TypeSubstitutor>,
    ) -> Option<LuaType> {
        match &self.extra {
            LuaTypeExtra::Alias {
                origin: Some(origin),
            } => {
                if substitutor.is_none() {
                    return Some(origin.clone());
                }

                let type_decl_id = self.get_id();
                if db
                    .get_type_index()
                    .get_generic_params(&type_decl_id)
                    .is_none()
                {
                    return Some(origin.clone());
                }

                let substitutor = substitutor.unwrap();
                Some(instantiate_type(db, &origin, substitutor))
            }
            _ => None,
        }
    }

    pub fn add_alias_origin(&mut self, replace: LuaType) {
        match &mut self.extra {
            LuaTypeExtra::Alias { origin, .. } => {
                *origin = Some(replace);
            }
            _ => {}
        }
    }

    pub fn add_enum_base(&mut self, base_type: LuaType) {
        match &mut self.extra {
            LuaTypeExtra::Enum { base } => {
                *base = Some(base_type);
            }
            _ => {}
        }
    }

    pub fn merge_decl(&mut self, other: LuaTypeDecl) {
        self.locations.extend(other.locations);
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct LuaTypeDeclId {
    id: ArcIntern<SmolStr>,
}

impl LuaTypeDeclId {
    #[allow(unused)]
    pub fn new_by_id(id: ArcIntern<SmolStr>) -> Self {
        Self { id }
    }

    pub fn new(str: &str) -> Self {
        Self {
            id: ArcIntern::new(SmolStr::new(str)),
        }
    }

    pub fn get_name(&self) -> &str {
        &self.id
    }

    pub fn get_simple_name(&self) -> &str {
        let basic_name = self.get_name();
        let just_name = if let Some(i) = basic_name.rfind('.') {
            &basic_name[i + 1..]
        } else {
            &basic_name
        };

        &just_name
    }

    pub fn collect_super_types(&self, db: &DbIndex, collected_types: &mut Vec<LuaType>) {
        // 必须广度优先
        let mut queue = Vec::new();
        queue.push(self.clone());

        while let Some(current_id) = queue.pop() {
            let super_types = db.get_type_index().get_super_types(&current_id);
            if let Some(super_types) = super_types {
                for super_type in super_types {
                    match &super_type {
                        LuaType::Ref(super_type_id) => {
                            if !collected_types.contains(&super_type) {
                                collected_types.push(super_type.clone());
                                queue.push(super_type_id.clone());
                            }
                        }
                        _ => {
                            if !collected_types.contains(&super_type) {
                                collected_types.push(super_type.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn collect_super_types_with_self(&self, db: &DbIndex, typ: LuaType) -> Vec<LuaType> {
        let mut collected_types: Vec<LuaType> = vec![typ];
        self.collect_super_types(db, &mut collected_types);
        collected_types
    }
}

impl Serialize for LuaTypeDeclId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.id)
    }
}

impl<'de> Deserialize<'de> for LuaTypeDeclId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(LuaTypeDeclId {
            id: ArcIntern::new(SmolStr::new(s)),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuaDeclLocation {
    pub file_id: FileId,
    pub range: TextRange,
    pub attrib: FlagSet<LuaTypeAttribute>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum LuaTypeExtra {
    Enum { base: Option<LuaType> },
    Class,
    Alias { origin: Option<LuaType> },
}
