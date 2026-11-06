//! Each test block's expected models, called macros and tested macros.

use sqlbuild_render::macro_calls::main::macro_call_names::macro_call_names;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::relationship_names::_helpers::cte_scan::{ScanText, expected_names};
use crate::relationship_names::constants::{
    DECLARATION_CALLS, MACRO_ACTUAL_CTE, MACRO_MODE, MODEL_MODE,
};
use crate::relationship_names::main::top_level_ctes::scan_top_level_ctes;
use crate::relationship_names::models::{
    RelationshipSource, TestBlockInput, TestFileInput, TopLevelCtes,
};
use crate::scope_index::models::{RelationshipFact, ResourceIdentity, ResourceKind};

/// Every block's expected models: model blocks scanned, other modes none.
pub(crate) fn scanned_names(
    file: &TestFileInput<'_>,
    syntax: &LexicalSyntax,
) -> Vec<Result<Vec<String>, String>> {
    file.blocks
        .iter()
        .map(|block| {
            if block.mode != MODEL_MODE {
                return Ok(Vec::new());
            }
            let text: ScanText<'_> = ScanText {
                sql: block.sql,
                file: file.relative_path,
                source: RelationshipSource::Test,
            };
            expected_names(&text, syntax)
        })
        .collect()
}

/// One block's fact: expected models, then nested macro calls, then a macro test's tested macros.
pub(crate) fn block_fact(
    block: &TestBlockInput<'_>,
    names: Result<Vec<String>, String>,
    relative_path: &str,
    syntax: &LexicalSyntax,
) -> Result<RelationshipFact, String> {
    let expected_models: Vec<String> = names?;
    let called_macros: Vec<String> = call_names(block.sql, true, syntax)?;
    let tested_macros: Vec<String> = if block.mode == MACRO_MODE {
        let ctes: Vec<(String, String)> =
            match scan_top_level_ctes(block.sql, relative_path, RelationshipSource::Test, syntax) {
                TopLevelCtes::Scanned(ctes) => ctes,
                TopLevelCtes::Failed(message) => return Err(message),
            };
        match ctes.iter().find(|(name, _)| name == MACRO_ACTUAL_CTE) {
            Some((_, body)) => call_names(body, false, syntax)?,
            None => Vec::new(),
        }
    } else {
        Vec::new()
    };
    Ok(RelationshipFact {
        resource: ResourceIdentity {
            kind: ResourceKind::Test,
            name: block.name.to_owned(),
        },
        expected_models,
        called_macros,
        tested_macros,
    })
}

/// `find_macro_call_names` or `find_nested_macro_call_names`, or the scan's message.
fn call_names(sql: &str, nested: bool, syntax: &LexicalSyntax) -> Result<Vec<String>, String> {
    let names: Vec<String> =
        macro_call_names(sql, nested, Some(syntax)).map_err(|error| error.message().to_owned())?;
    Ok(names
        .into_iter()
        .filter(|name| !DECLARATION_CALLS.contains(&name.as_str()))
        .collect())
}
