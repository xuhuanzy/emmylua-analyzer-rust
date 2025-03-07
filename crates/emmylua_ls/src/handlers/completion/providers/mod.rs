mod auto_require_provider;
mod doc_name_token_provider;
mod doc_tag_provider;
mod doc_type_provider;
mod env_provider;
mod file_path_provider;
mod keywords_provider;
mod member_provider;
mod module_path_provider;
mod postfix_provider;
mod table_decl_field_provider;
mod type_special_provider;

use super::completion_builder::CompletionBuilder;

pub fn add_completions(builder: &mut CompletionBuilder) -> Option<()> {
    module_path_provider::add_completion(builder);
    file_path_provider::add_completion(builder);
    keywords_provider::add_completion(builder);
    type_special_provider::add_completion(builder);
    env_provider::add_completion(builder);
    member_provider::add_completion(builder);
    table_decl_field_provider::add_completion(builder);
    auto_require_provider::add_completion(builder);
    doc_tag_provider::add_completion(builder);
    doc_type_provider::add_completion(builder);
    doc_name_token_provider::add_completion(builder);
    postfix_provider::add_completion(builder);

    for (index, item) in builder.get_completion_items_mut().iter_mut().enumerate() {
        if item.sort_text.is_none() {
            item.sort_text = Some(format!("{:04}", index));
        }
    }

    Some(())
}
