use emmylua_code_analysis::{
    DbIndex, LuaFunctionType, LuaMember, LuaMemberKey, LuaMemberOwner, LuaPropertyOwnerId,
    LuaSignature, LuaSignatureId, LuaType, OneOrMulti, RenderLevel,
};

use emmylua_code_analysis::humanize_type;
use itertools::Itertools;

use super::hover_builder::HoverBuilder;

pub fn hover_const_type(db: &DbIndex, typ: &LuaType) -> String {
    let const_value = humanize_type(db, typ, RenderLevel::Detailed);

    match typ {
        LuaType::IntegerConst(_) | LuaType::DocIntegerConst(_) => {
            format!("integer = {}", const_value)
        }
        LuaType::FloatConst(_) => format!("number = {}", const_value),
        LuaType::StringConst(_) | LuaType::DocStringConst(_) => format!("string = {}", const_value),
        _ => const_value,
    }
}

pub fn hover_function_type(
    builder: &mut HoverBuilder,
    db: &DbIndex,
    typ: &LuaType,
    function_member: Option<&LuaMember>,
    func_name: &str,
) {
    match typ {
        LuaType::Function => builder.set_type_description(format!("function {}()", func_name)),
        LuaType::DocFunction(lua_func) => {
            hover_doc_function_type(builder, db, &lua_func, function_member, func_name);
        }
        LuaType::Signature(signature_id) => hover_signature_type(
            builder,
            db,
            signature_id.clone(),
            function_member,
            func_name,
        )
        .unwrap_or_else(|| {
            builder.set_type_description(format!("function {}", func_name));
            builder.signature_overload = None;
        }),
        _ => builder.set_type_description(format!("function {}", func_name)),
    }
}

fn hover_doc_function_type(
    builder: &mut HoverBuilder,
    db: &DbIndex,
    lua_func: &LuaFunctionType,
    function_member: Option<&LuaMember>,
    func_name: &str,
) {
    if let Some(owner_member) = function_member {
        let member_one = db
            .get_member_index()
            .get_member_by_key(owner_member.get_owner(), owner_member.get_key().clone());

        if let Some(OneOrMulti::Multi(ids)) = member_one.and_then(|m| Some(m)) {
            let mut docfunctions = Vec::new();
            for id in ids.iter() {
                if let Some(member) = db.get_member_index().get_member(id) {
                    if let LuaType::DocFunction(overload_fun) = member.get_decl_type() {
                        docfunctions.push((overload_fun.as_ref().clone(), member));
                    }
                }
            }

            if let Some(call_function) = builder.get_call_function() {
                for (func, member) in &docfunctions {
                    if *func == call_function {
                        let result =
                            build_doc_function_type(db, &call_function, Some(member), func_name);
                        builder.set_type_description(result);
                        return; // 此时只需要显示完全匹配的签名
                    }
                }
            }
            // 需要过滤掉重复的
            let mut overloads = Vec::new();
            for (func, member) in &docfunctions {
                overloads.push(build_doc_function_type(db, func, Some(member), func_name));
            }
            overloads = overloads.into_iter().unique().collect();
            for (i, overload) in overloads.iter().enumerate() {
                if i == 0 {
                    builder.set_type_description(overload.clone());
                } else {
                    builder.add_signature_overload(overload.clone());
                }
            }
            return;
        }
    }
    builder.set_type_description(build_doc_function_type(
        db,
        lua_func,
        function_member,
        func_name,
    ));
}

fn build_doc_function_type(
    db: &DbIndex,
    lua_func: &LuaFunctionType,
    owner_member: Option<&LuaMember>,
    func_name: &str,
) -> String {
    let async_label = if lua_func.is_async() { "async " } else { "" };
    let mut type_label = "function ";
    // 有可能来源于类. 例如: `local add = class.add`, `add()`应被视为类方法
    let full_name = if let Some(owner_member) = owner_member {
        let mut name = String::new();
        let parent_owner = owner_member.get_owner();
        if let LuaMemberOwner::Type(ty) = &parent_owner.clone() {
            name.push_str(ty.get_simple_name());
            if owner_member.is_field().is_some() {
                type_label = "(field) ";
            }
        }
        match owner_member.get_decl_type() {
            LuaType::DocFunction(func) => {
                if func.is_colon_define()
                    || func.get_params().first().and_then(|param| {
                        param.1.as_ref().map(|ty| {
                            param.0 == "self"
                                && humanize_type(db, ty, RenderLevel::Normal) == "self"
                        })
                    }) == Some(true)
                {
                    type_label = "(method) ";
                    name.push_str(":");
                } else {
                    name.push_str(".");
                }
            }
            _ => {}
        }
        if let LuaMemberKey::Name(n) = owner_member.get_key() {
            name.push_str(n.as_str());
        }
        name
    } else {
        func_name.to_string()
    };

    let params = lua_func
        .get_params()
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let name = param.0.clone();
            if index == 0
                && param.1.is_some()
                && name == "self"
                && humanize_type(db, param.1.as_ref().unwrap(), RenderLevel::Normal) == "self"
            {
                "".to_string()
            } else if let Some(ty) = &param.1 {
                format!("{}: {}", name, humanize_type(db, ty, RenderLevel::Normal))
            } else {
                name.to_string()
            }
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ");

    let rets = {
        let rets = lua_func.get_ret();
        if rets.is_empty() {
            "".to_string()
        } else {
            format!(
                " -> {}",
                rets.iter()
                    .map(|ty| humanize_type(db, ty, RenderLevel::Simple))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    };
    format_function_type(type_label, async_label, full_name, params, rets)
}

fn hover_signature_type(
    builder: &mut HoverBuilder,
    db: &DbIndex,
    signature_id: LuaSignatureId,
    owner_member: Option<&LuaMember>,
    func_name: &str,
) -> Option<()> {
    let signature = db.get_signature_index().get(&signature_id)?;

    let mut type_label = "function ";
    // 有可能来源于类. 例如: `local add = class.add`, `add()`应被视为类定义的内容
    let full_name = if let Some(owner_member) = owner_member {
        let mut name = String::new();
        if let LuaMemberOwner::Type(ty) = &owner_member.get_owner() {
            name.push_str(ty.get_simple_name());
            if signature.is_colon_define {
                type_label = "(method) ";
                name.push_str(":");
            } else {
                name.push_str(".");
            }
        }
        if let LuaMemberKey::Name(n) = owner_member.get_key() {
            name.push_str(n.as_str());
        }
        name
    } else {
        func_name.to_string()
    };

    // 构建 signature
    let signature_info = {
        let async_label = db
            .get_property_index()
            .get_property(LuaPropertyOwnerId::Signature(signature_id))
            .map(|prop| if prop.is_async { "async " } else { "" })
            .unwrap_or("");
        let params = signature
            .get_type_params()
            .iter()
            .map(|param| {
                let name = param.0.clone();
                if let Some(ty) = &param.1 {
                    format!("{}: {}", name, humanize_type(db, ty, RenderLevel::Simple))
                } else {
                    name
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let rets = get_signature_rets_string(db, signature, builder.is_completion, None);
        // 构建签名
        format_function_type(type_label, async_label, full_name.clone(), params, rets)
    };
    // 构建所有重载
    let overloads: Vec<String> = {
        let call_function = builder.get_call_function();
        dbg!(&call_function);
        let mut overloads = Vec::new();
        for (_, overload) in signature.overloads.iter().enumerate() {
            let async_label = if overload.is_async() { "async " } else { "" };
            let params = overload
                .get_params()
                .iter()
                .map(|param| {
                    let name = param.0.clone();
                    if let Some(ty) = &param.1 {
                        format!("{}: {}", name, humanize_type(db, ty, RenderLevel::Simple))
                    } else {
                        name
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            let rets =
                get_signature_rets_string(db, signature, builder.is_completion, Some(overload));

            let result =
                format_function_type(type_label, async_label, full_name.clone(), params, rets);

            if let Some(call_function) = &call_function {
                if *call_function == **overload {
                    // 如果具有完全匹配的签名, 那么将其设置为当前签名, 且不显示重载
                    builder.set_type_description(result);
                    builder.signature_overload = None;
                    return Some(());
                }
            };
            overloads.push(result);
        }
        overloads
    };

    builder.set_type_description(signature_info);
    for overload in overloads {
        builder.add_signature_overload(overload);
    }
    Some(())
}

fn get_signature_rets_string(
    db: &DbIndex,
    signature: &LuaSignature,
    is_completion: bool,
    overload: Option<&LuaFunctionType>,
) -> String {
    let mut result = String::new();
    // overload 的返回值固定为单行
    let overload_rets_string = if let Some(overload) = overload {
        let rets = overload.get_ret();
        let rets_string = rets
            .iter()
            .map(|typ| humanize_type(db, typ, RenderLevel::Simple))
            .collect::<Vec<_>>()
            .join(", ");
        if rets_string.is_empty() {
            "".to_string()
        } else {
            format!(" -> {}", rets_string)
        }
    } else {
        "".to_string()
    };

    if is_completion {
        let rets = if !overload_rets_string.is_empty() {
            overload_rets_string
        } else {
            let rets = &signature.return_docs;
            if rets.is_empty() {
                "".to_string()
            } else {
                format!(
                    " -> {}",
                    rets.iter()
                        .map(|ret| humanize_type(db, &ret.type_ref, RenderLevel::Simple))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        };
        result.push_str(rets.as_str());
    } else {
        let rets = if !overload_rets_string.is_empty() {
            overload_rets_string
        } else {
            let rets = &signature.return_docs;
            if rets.is_empty() {
                "".to_string()
            } else {
                let mut rets_string_multiline = String::with_capacity(512); // 预分配容量
                rets_string_multiline.push_str("\n");

                for (i, ret) in rets.iter().enumerate() {
                    let type_text = humanize_type(db, &ret.type_ref, RenderLevel::Simple);
                    let prefix = if i == 0 {
                        "->".to_string()
                    } else {
                        format!("{}.", i + 1)
                    };
                    let name = ret.name.clone().unwrap_or_default();
                    let detail = ret
                        .description
                        .as_ref()
                        .map(|desc| format!(" — {}", desc.trim_end()))
                        .unwrap_or_default();

                    rets_string_multiline.push_str(&format!(
                        "  {}{} {}{}\n",
                        prefix,
                        if !name.is_empty() {
                            format!("{}:", name)
                        } else {
                            "".to_string()
                        },
                        type_text,
                        detail
                    ));
                }
                rets_string_multiline
            }
        };
        result.push_str(rets.as_str());
    };
    result
}

fn format_function_type(
    type_label: &str,
    async_label: &str,
    full_name: String,
    params: String,
    rets: String,
) -> String {
    let prefix = if type_label.starts_with("function") {
        format!("{}{}", async_label, type_label)
    } else {
        format!("{}{}", type_label, async_label)
    };
    format!("{}{}({}){}", prefix, full_name, params, rets)
}
