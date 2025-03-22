use std::collections::HashMap;

use smol_str::SmolStr;

use crate::{
    semantic::{
        generic::{instantiate_type_generic, TypeSubstitutor},
        InferGuard,
    },
    DbIndex, FileId, LuaGenericType, LuaInstanceType, LuaIntersectionType, LuaMemberKey,
    LuaMemberOwner, LuaObjectType, LuaSemanticDeclId, LuaTupleType, LuaType, LuaTypeDeclId,
    LuaUnionType,
};

use super::{get_buildin_type_map_type_id, InferMembersResult, LuaMemberInfo};

pub fn infer_members(db: &DbIndex, prefix_type: &LuaType) -> InferMembersResult {
    infer_members_guard(db, prefix_type, &mut InferGuard::new())
}

pub fn infer_members_guard(
    db: &DbIndex,
    prefix_type: &LuaType,
    infer_guard: &mut InferGuard,
) -> InferMembersResult {
    match &prefix_type {
        LuaType::TableConst(id) => {
            let member_owner = LuaMemberOwner::Element(id.clone());
            infer_normal_members(db, member_owner)
        }
        LuaType::String | LuaType::Io | LuaType::StringConst(_) => {
            let type_decl_id = get_buildin_type_map_type_id(&prefix_type)?;
            infer_custom_type_members(db, &type_decl_id, infer_guard)
        }
        LuaType::Ref(type_decl_id) => infer_custom_type_members(db, type_decl_id, infer_guard),
        LuaType::Def(type_decl_id) => infer_custom_type_members(db, type_decl_id, infer_guard),
        // // LuaType::Module(_) => todo!(),
        LuaType::Tuple(tuple_type) => infer_tuple_members(tuple_type),
        LuaType::Object(object_type) => infer_object_members(object_type),
        LuaType::Union(union_type) => infer_union_members(db, union_type, infer_guard),
        LuaType::Intersection(intersection_type) => {
            infer_intersection_members(db, intersection_type, infer_guard)
        }
        LuaType::Generic(generic_type) => infer_generic_members(db, generic_type, infer_guard),
        LuaType::Global => infer_global_members(db),
        LuaType::Instance(inst) => infer_instance_members(db, inst, infer_guard),
        LuaType::Namespace(ns) => infer_namespace_members(db, ns),
        _ => None,
    }
}

fn infer_normal_members(db: &DbIndex, member_owner: LuaMemberOwner) -> InferMembersResult {
    let mut members = Vec::new();
    let member_index = db.get_member_index();
    let owner_members = member_index.get_members(&member_owner)?;
    for member in owner_members {
        members.push(LuaMemberInfo {
            property_owner_id: Some(LuaSemanticDeclId::Member(member.get_id())),
            key: member.get_key().clone(),
            typ: member.get_decl_type(),
            feature: Some(member.get_feature()),
            overload_index: None,
        });
    }

    Some(members)
}

fn infer_custom_type_members(
    db: &DbIndex,
    type_decl_id: &LuaTypeDeclId,
    infer_guard: &mut InferGuard,
) -> InferMembersResult {
    infer_guard.check(&type_decl_id)?;
    let type_index = db.get_type_index();
    let type_decl = type_index.get_type_decl(&type_decl_id)?;
    if type_decl.is_alias() {
        if let Some(origin) = type_decl.get_alias_origin(db, None) {
            return infer_members_guard(db, &origin, infer_guard);
        } else {
            return infer_members_guard(db, &LuaType::String, infer_guard);
        }
    }

    let mut members = Vec::new();
    let member_index = db.get_member_index();
    if let Some(type_members) =
        member_index.get_members(&LuaMemberOwner::Type(type_decl_id.clone()))
    {
        for member in type_members {
            members.push(LuaMemberInfo {
                property_owner_id: Some(LuaSemanticDeclId::Member(member.get_id())),
                key: member.get_key().clone(),
                typ: member.get_decl_type(),
                feature: Some(member.get_feature()),
                overload_index: None,
            });
        }
    }

    if type_decl.is_class() {
        if let Some(super_types) = type_index.get_super_types(&type_decl_id) {
            for super_type in super_types {
                if let Some(super_members) = infer_members_guard(db, &super_type, infer_guard) {
                    members.extend(super_members);
                }
            }
        }
    }

    Some(members)
}

fn infer_tuple_members(tuple_type: &LuaTupleType) -> InferMembersResult {
    let mut members = Vec::new();
    for (idx, typ) in tuple_type.get_types().iter().enumerate() {
        members.push(LuaMemberInfo {
            property_owner_id: None,
            key: LuaMemberKey::Integer((idx + 1) as i64),
            typ: typ.clone(),
            feature: None,
            overload_index: None,
        });
    }

    Some(members)
}

fn infer_object_members(object_type: &LuaObjectType) -> InferMembersResult {
    let mut members = Vec::new();
    for (key, typ) in object_type.get_fields().iter() {
        members.push(LuaMemberInfo {
            property_owner_id: None,
            key: key.clone(),
            typ: typ.clone(),
            feature: None,
            overload_index: None,
        });
    }

    Some(members)
}

fn infer_union_members(
    db: &DbIndex,
    union_type: &LuaUnionType,
    infer_guard: &mut InferGuard,
) -> InferMembersResult {
    let mut members = Vec::new();
    for typ in union_type.get_types().iter() {
        let sub_members = infer_members_guard(db, typ, infer_guard);
        if let Some(sub_members) = sub_members {
            members.extend(sub_members);
        }
    }

    Some(members)
}

fn infer_intersection_members(
    db: &DbIndex,
    intersection_type: &LuaIntersectionType,
    infer_guard: &mut InferGuard,
) -> InferMembersResult {
    let mut members = Vec::new();
    for typ in intersection_type.get_types().iter() {
        let sub_members = infer_members_guard(db, typ, infer_guard);
        if let Some(sub_members) = sub_members {
            members.push(sub_members);
        }
    }

    if members.is_empty() {
        return None;
    } else if members.len() == 1 {
        return Some(members.remove(0));
    } else {
        let mut result = Vec::new();
        let mut member_count = HashMap::new();

        for member in members.iter().flatten() {
            let key = member.key.clone();
            let typ = member.typ.clone();
            let entry = member_count.entry((key, typ)).or_insert(1);
            *entry += 1;
        }

        for ((key, typ), count) in member_count {
            if count >= members.len() {
                result.push(LuaMemberInfo {
                    property_owner_id: None,
                    key,
                    typ,
                    feature: None,
                    overload_index: None,
                });
            }
        }

        return Some(result);
    }
}

fn infer_generic_members(
    db: &DbIndex,
    generic_type: &LuaGenericType,
    infer_guard: &mut InferGuard,
) -> InferMembersResult {
    let base_type = generic_type.get_base_type();
    let mut members = infer_members_guard(db, &base_type, infer_guard)?;

    let generic_params = generic_type.get_params();
    let substitutor = TypeSubstitutor::from_type_array(generic_params.clone());
    for info in members.iter_mut() {
        info.typ = instantiate_type_generic(db, &info.typ, &substitutor);
    }

    Some(members)
}

fn infer_global_members(db: &DbIndex) -> InferMembersResult {
    let mut members = Vec::new();
    let decl_index = db.get_decl_index();
    let global_decls = decl_index.get_global_decls();
    for decl_id in global_decls {
        let decl = decl_index.get_decl(&decl_id)?;
        members.push(LuaMemberInfo {
            property_owner_id: Some(LuaSemanticDeclId::LuaDecl(decl_id)),
            key: LuaMemberKey::Name(decl.get_name().to_string().into()),
            typ: decl.get_type().cloned().unwrap_or(LuaType::Unknown),
            feature: None,
            overload_index: None,
        });
    }

    Some(members)
}

fn infer_instance_members(
    db: &DbIndex,
    inst: &LuaInstanceType,
    infer_guard: &mut InferGuard,
) -> InferMembersResult {
    let mut members = Vec::new();
    let range = inst.get_range();
    let member_owner = LuaMemberOwner::Element(range.clone());
    if let Some(normal_members) = infer_normal_members(db, member_owner) {
        members.extend(normal_members);
    }

    let origin_type = inst.get_base();
    if let Some(origin_members) = infer_members_guard(db, origin_type, infer_guard) {
        members.extend(origin_members);
    }

    Some(members)
}

fn infer_namespace_members(db: &DbIndex, ns: &str) -> InferMembersResult {
    let mut members = Vec::new();

    let prefix = format!("{}.", ns);
    let type_index = db.get_type_index();
    let type_decl_id_map = type_index.find_type_decls(FileId::VIRTUAL, &prefix);
    for (name, type_decl_id) in type_decl_id_map {
        if let Some(type_decl_id) = type_decl_id {
            let typ = LuaType::Def(type_decl_id.clone());
            let property_owner_id = LuaSemanticDeclId::TypeDecl(type_decl_id);
            members.push(LuaMemberInfo {
                property_owner_id: Some(property_owner_id),
                key: LuaMemberKey::Name(name.into()),
                typ,
                feature: None,
                overload_index: None,
            });
        } else {
            members.push(LuaMemberInfo {
                property_owner_id: None,
                key: LuaMemberKey::Name(name.clone().into()),
                typ: LuaType::Namespace(SmolStr::new(format!("{}.{}", ns, &name)).into()),
                feature: None,
                overload_index: None,
            });
        }
    }

    Some(members)
}
