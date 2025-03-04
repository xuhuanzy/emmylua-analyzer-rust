use emmylua_parser::{
    LuaAstNode, LuaAstToken, LuaBlock, LuaClosureExpr, LuaReturnStat, LuaTokenKind,
};

use crate::{humanize_type, DiagnosticCode, LuaSignatureId, LuaType, RenderLevel, SemanticModel};

use super::DiagnosticContext;

pub const CODES: &[DiagnosticCode] = &[DiagnosticCode::ReturnTypeMismatch];

pub fn check(context: &mut DiagnosticContext, semantic_model: &SemanticModel) -> Option<()> {
    let root = semantic_model.get_root().clone();
    for return_stat in root.descendants::<LuaReturnStat>() {
        check_return_stat(context, semantic_model, &return_stat);
    }
    Some(())
}

fn check_return_stat(
    context: &mut DiagnosticContext,
    semantic_model: &SemanticModel,
    return_stat: &LuaReturnStat,
) -> Option<()> {
    let db = semantic_model.get_db();
    let closure_expr = return_stat
        .get_parent::<LuaBlock>()?
        .ancestors::<LuaClosureExpr>()
        .next()?;

    let signature_id = LuaSignatureId::from_closure(semantic_model.get_file_id(), &closure_expr);
    let signature = context.db.get_signature_index().get(&signature_id)?;
    let return_types = signature.get_return_types();

    // 处理最后一个返回值类型为 `...`, 但这个判断似乎不可靠?
    let disable_return_count_check = if let Some(return_type) = return_types.last() {
        return_type.is_unknown()
    } else {
        false
    };

    let expr_return_len = return_stat.get_expr_list().collect::<Vec<_>>().len();
    let return_types_len = return_types.len();
    if !disable_return_count_check && expr_return_len < return_types_len {
        context.add_diagnostic(
            DiagnosticCode::MissingReturnValue,
            return_stat
                .token_by_kind(LuaTokenKind::TkReturn)?
                .get_range(),
            t!(
                "Annotations specify that at least %{min} return value(s) are required, found %{rmin} returned here instead.",
                min = return_types_len,
                rmin = expr_return_len
            )
            .to_string(),
            None,
        );
    }

    for (idx, expr) in return_stat.get_expr_list().enumerate() {
        if !disable_return_count_check && idx >= return_types_len {
            context.add_diagnostic(
                DiagnosticCode::RedundantReturnValue,
                expr.get_range(),
                t!(
                    "Annotations specify that at most %{max} return value(s) are required, found %{rmax} returned here instead.",
                    max = return_types_len,
                    rmax = expr_return_len
                )
                .to_string(),
                None,
            );
        }

        let expr_type = semantic_model
            .infer_expr(expr.clone())
            .unwrap_or(LuaType::Any);
        let return_type = return_types.get(idx).unwrap_or(&LuaType::Any);
        let result = semantic_model.type_check(&return_type, &expr_type);
        match result {
            Ok(_) => {}
            Err(_) => {
                context.add_diagnostic(
                    DiagnosticCode::ReturnTypeMismatch,
                    expr.get_range(),
                    t!(
                        "Annotations specify that return value %{index} has a type of `%{source}`, returning value of type `%{found}` here instead.",
                        index = idx + 1,
                        source = humanize_type(db, &return_type, RenderLevel::Simple),
                        found = humanize_type(db, &expr_type, RenderLevel::Simple)
                    )
                    .to_string(),
                    None,
                );
            }
        }
    }
    Some(())
}

// #[allow(unused)]
// fn check_closure_expr(
//     context: &mut DiagnosticContext,
//     semantic_model: &SemanticModel,
//     closure_expr: &LuaClosureExpr,
// ) -> Option<()> {
//     let db = semantic_model.get_db();

//     let signature_id = LuaSignatureId::from_closure(semantic_model.get_file_id(), &closure_expr);
//     let signature = context.db.get_signature_index().get(&signature_id)?;
//     let return_types = signature.get_return_types();

//     // 处理最后一个返回值类型为 `...`, 但这个判断似乎不可靠?
//     let disable_return_count_check = if let Some(return_type) = return_types.last() {
//         return_type.is_unknown()
//     } else {
//         false
//     };

//     let block = closure_expr.get_block()?;

//     for stat in block.children::<LuaReturnStat>() {
//         let expr_return_len = stat.get_expr_list().collect::<Vec<_>>().len();
//         let return_types_len = return_types.len();
//         if !disable_return_count_check && expr_return_len < return_types_len {
//             context.add_diagnostic(
//                 DiagnosticCode::MissingReturnValue,
//                 stat.token_by_kind(LuaTokenKind::TkReturn)?.get_range(),
//                 t!(
//                     "至少需要 %{len} 个返回值，但此处只返回了 %{expr_len} 个",
//                     len = return_types_len,
//                     expr_len = expr_return_len
//                 )
//                 .to_string(),
//                 None,
//             );
//         }

//         for (idx, expr) in stat.get_expr_list().enumerate() {
//             if !disable_return_count_check && idx >= return_types_len {
//                 context.add_diagnostic(
//                     DiagnosticCode::RedundantReturnValue,
//                     expr.get_range(),
//                     t!(
//                         "最多只有 %{len} 个返回值，但此处返回了 %{expr_len} 个",
//                         len = return_types_len,
//                         expr_len = expr_return_len
//                     )
//                     .to_string(),
//                     None,
//                 );
//             }

//             let expr_type = semantic_model
//                 .infer_expr(expr.clone())
//                 .unwrap_or(LuaType::Any);
//             let return_type = return_types.get(idx).unwrap_or(&LuaType::Any);
//             let result = semantic_model.type_check(&return_type, &expr_type);
//             match result {
//                 Ok(_) => {}
//                 Err(_) => {
//                     context.add_diagnostic(
//                         DiagnosticCode::ReturnTypeMismatch,
//                         expr.get_range(),
//                         t!(
//                             "第 %{idx} 个返回值的类型为 `%{source}`，但实际返回类型为 `%{found}`",
//                             idx = idx + 1,
//                             source = humanize_type(db, &return_type, RenderLevel::Simple),
//                             found = humanize_type(db, &expr_type, RenderLevel::Simple)
//                         )
//                         .to_string(),
//                         None,
//                     );
//                 }
//             }
//         }
//     }

//     Some(())
// }


