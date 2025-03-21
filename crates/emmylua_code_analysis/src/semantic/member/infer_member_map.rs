use std::collections::HashMap;

use crate::{DbIndex, LuaMemberKey, LuaType};

use super::{
    infer_members::{self},
    LuaMemberInfo,
};

pub fn infer_member_map(
    db: &DbIndex,
    prefix_type: &LuaType,
) -> Option<HashMap<LuaMemberKey, Vec<LuaMemberInfo>>> {
    let members = infer_members::infer_members(db, prefix_type)?;

    let mut member_map = HashMap::new();
    for member in members {
        let key = member.key.clone();
        let typ = &member.typ;
        if let LuaType::Union(u) = typ {
            if u.get_types().iter().all(|f| f.is_function()) {
                for f in u.get_types() {
                    let new_member = LuaMemberInfo {
                        key: key.clone(),
                        typ: f.clone(),
                        property_owner_id: member.property_owner_id.clone(),
                        feature: member.feature.clone(),
                    };

                    member_map
                        .entry(key.clone())
                        .or_insert_with(Vec::new)
                        .push(new_member);
                }
                continue;
            }
        }
        member_map.entry(key).or_insert_with(Vec::new).push(member);
    }

    Some(member_map)
}
