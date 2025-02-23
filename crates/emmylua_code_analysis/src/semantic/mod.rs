mod call_func;
mod infer;
mod instantiate;
mod member;
mod overload_resolve;
mod reference;
mod semantic_info;
mod type_check;
mod visibility;

use std::cell::RefCell;
use std::{collections::HashSet, sync::Arc};

use emmylua_parser::{LuaCallExpr, LuaChunk, LuaExpr, LuaSyntaxNode, LuaSyntaxToken, LuaTableExpr};
pub use infer::LuaInferConfig;
use infer::{infer_table_should_be, InferResult};
use member::infer_members;
pub use member::LuaMemberInfo;
use reference::is_reference_to;
use rowan::{NodeOrToken, TextRange};
pub use semantic_info::SemanticInfo;
use semantic_info::{
    infer_node_property_owner, infer_node_semantic_info, infer_token_property_owner,
    infer_token_semantic_info,
};
pub(crate) use type_check::check_type_compact;
use type_check::is_sub_type_of;
use visibility::check_visibility;

use crate::{LuaFunctionType, LuaMember};
use crate::{db_index::LuaTypeDeclId, Emmyrc, LuaDocument, LuaPropertyOwnerId};
use crate::{
    db_index::{DbIndex, LuaType},
    FileId,
};
pub(crate) use call_func::infer_call_expr_func;
pub(crate) use infer::infer_expr;
pub use instantiate::{instantiate_type, TypeSubstitutor};
use overload_resolve::resolve_signature;
pub use type_check::{TypeCheckFailReason, TypeCheckResult};

#[derive(Debug)]
pub struct SemanticModel<'a> {
    file_id: FileId,
    db: &'a DbIndex,
    infer_config: RefCell<LuaInferConfig>,
    emmyrc: Arc<Emmyrc>,
    root: LuaChunk,
}

unsafe impl<'a> Send for SemanticModel<'a> {}
unsafe impl<'a> Sync for SemanticModel<'a> {}

impl<'a> SemanticModel<'a> {
    pub fn new(
        file_id: FileId,
        db: &'a DbIndex,
        infer_config: LuaInferConfig,
        emmyrc: Arc<Emmyrc>,
        root: LuaChunk,
    ) -> Self {
        Self {
            file_id,
            db,
            infer_config: RefCell::new(infer_config),
            emmyrc,
            root,
        }
    }

    pub fn get_document(&self) -> LuaDocument {
        self.db.get_vfs().get_document(&self.file_id).unwrap()
    }

    pub fn get_document_by_file_id(&self, file_id: FileId) -> Option<LuaDocument> {
        self.db.get_vfs().get_document(&file_id)
    }

    pub fn get_root_by_file_id(&self, file_id: FileId) -> Option<LuaChunk> {
        Some(
            self.db
                .get_vfs()
                .get_syntax_tree(&file_id)?
                .get_chunk_node(),
        )
    }

    pub fn get_file_parse_error(&self) -> Option<Vec<(String, TextRange)>> {
        self.db.get_vfs().get_file_parse_error(&self.file_id)
    }

    pub fn infer_expr(&self, expr: LuaExpr) -> InferResult {
        infer_expr(self.db, &mut self.infer_config.borrow_mut(), expr)
    }

    pub fn infer_table_should_be(&self, table: LuaTableExpr) -> Option<LuaType> {
        infer_table_should_be(self.db, &mut self.infer_config.borrow_mut(), table)
    }

    pub fn infer_member_infos(&self, prefix_type: &LuaType) -> Option<Vec<LuaMemberInfo>> {
        infer_members(self.db, prefix_type)
    }

    pub fn type_check(&self, source: &LuaType, compact_type: &LuaType) -> TypeCheckResult {
        check_type_compact(self.db, source, compact_type)
    }

    pub fn infer_call_expr_func(
        &self,
        call_expr: LuaCallExpr,
        arg_count: Option<usize>,
    ) -> Option<Arc<LuaFunctionType>> {
        let prefix_expr = call_expr.get_prefix_expr()?;
        let call_expr_type = infer_expr(
            self.db,
            &mut self.infer_config.borrow_mut(),
            prefix_expr.into(),
        )?;
        infer_call_expr_func(
            self.db,
            &mut self.infer_config.borrow_mut(),
            call_expr,
            call_expr_type,
            &mut InferGuard::new(),
            arg_count,
        )
    }

    pub fn get_semantic_info(
        &self,
        node_or_token: NodeOrToken<LuaSyntaxNode, LuaSyntaxToken>,
    ) -> Option<SemanticInfo> {
        match node_or_token {
            NodeOrToken::Node(node) => {
                infer_node_semantic_info(self.db, &mut self.infer_config.borrow_mut(), node)
            }
            NodeOrToken::Token(token) => {
                infer_token_semantic_info(self.db, &mut self.infer_config.borrow_mut(), token)
            }
        }
    }

    pub fn get_property_owner_id(
        &self,
        node_or_token: NodeOrToken<LuaSyntaxNode, LuaSyntaxToken>,
    ) -> Option<LuaPropertyOwnerId> {
        match node_or_token {
            NodeOrToken::Node(node) => {
                infer_node_property_owner(self.db, &mut self.infer_config.borrow_mut(), node)
            }
            NodeOrToken::Token(token) => {
                infer_token_property_owner(self.db, &mut self.infer_config.borrow_mut(), token)
            }
        }
    }

    pub fn is_reference_to(&self, node: LuaSyntaxNode, property_owner: LuaPropertyOwnerId) -> bool {
        is_reference_to(
            self.db,
            &mut self.infer_config.borrow_mut(),
            node,
            property_owner,
        )
        .unwrap_or(false)
    }

    pub fn is_property_visible(
        &self,
        token: LuaSyntaxToken,
        property_owner: LuaPropertyOwnerId,
    ) -> bool {
        check_visibility(
            self.db,
            self.file_id,
            &self.emmyrc,
            &mut self.infer_config.borrow_mut(),
            token,
            property_owner,
        )
        .unwrap_or(true)
    }

    pub fn is_sub_type_of(
        &mut self,
        sub_type_ref_id: &LuaTypeDeclId,
        super_type_ref_id: &LuaTypeDeclId,
    ) -> bool {
        is_sub_type_of(self.db, sub_type_ref_id, super_type_ref_id)
    }

    pub fn get_emmyrc(&self) -> &Emmyrc {
        &self.emmyrc
    }

    pub fn get_root(&self) -> &LuaChunk {
        &self.root
    }

    pub fn get_db(&self) -> &DbIndex {
        self.db
    }

    pub fn get_file_id(&self) -> FileId {
        self.file_id
    }

    pub fn get_config(&self) -> &RefCell<LuaInferConfig> {
        &self.infer_config
    }
}

/// Guard to prevent infinite recursion
/// Some type may reference itself, so we need to check if we have already inferred this type
#[derive(Debug)]
pub struct InferGuard {
    guard: HashSet<LuaTypeDeclId>,
}

impl InferGuard {
    pub fn new() -> Self {
        Self {
            guard: HashSet::default(),
        }
    }

    pub fn check(&mut self, type_id: &LuaTypeDeclId) -> Option<()> {
        if self.guard.contains(type_id) {
            return None;
        }
        self.guard.insert(type_id.clone());
        Some(())
    }
}
