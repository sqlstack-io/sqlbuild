use std::sync::Arc;

use crate::sql_macros::_helpers::rendering::expansion::expand_text;
use crate::sql_macros::errors::ExpansionFailure;
use crate::sql_macros::errors::SqlMacroError;
use crate::sql_macros::main::parse_sql_macro::parse_sql_macro;
use crate::sql_macros::models::{
    ExpandedText, RenderedDeclarations, SqlMacroDefinition, TextContext, TextSplice, TypedCall,
};
use crate::sql_macros::tests::test_types::{AsciiArguments, FakeHost};
use crate::sql_macros::types::{ConstantShape, SqlMacroHost, SqlMacroScope};

/// Constants and enum members the fake host renders, by reference text.
const DECLARATIONS: [(&str, &str); 4] = [
    ("@const(\"supported_countries\")", "('GB', 'FR', 'HK')"),
    ("@const(\"price_places\")", "2"),
    ("@enum(\"order_status\").CLOSED", "'closed'"),
    ("@enum(\"order_status\").OPEN", "'open'"),
];

impl FakeHost {
    pub(super) fn new(macros: &[(&str, &str)], adapter: &'static str) -> Self {
        let definitions = macros
            .iter()
            .map(|(name, source)| {
                let path: String = format!("macros/sql/{name}.sql");
                let definition = parse_sql_macro(name, &path, source).expect("valid macro");
                ((*name).to_owned(), Arc::new(definition))
            })
            .collect();
        Self {
            definitions,
            adapter,
        }
    }
}

impl SqlMacroHost for FakeHost {
    type Error = String;

    fn definition(
        &self,
        _scope: &SqlMacroScope,
        name: &str,
    ) -> Result<Arc<SqlMacroDefinition>, ExpansionFailure<String>> {
        self.definitions
            .get(name)
            .cloned()
            .ok_or_else(|| ExpansionFailure::Host(format!("unknown macro {name}")))
    }

    fn render_declarations(
        &self,
        _scope: &SqlMacroScope,
        text: &str,
    ) -> Result<RenderedDeclarations, ExpansionFailure<String>> {
        let mut rendered: RenderedDeclarations = RenderedDeclarations::default();
        let mut cursor: usize = 0;
        while let Some((start, reference, value)) = DECLARATIONS
            .iter()
            .filter_map(|(reference, value)| {
                text[cursor..]
                    .find(reference)
                    .map(|found| (cursor + found, *reference, *value))
            })
            .min_by_key(|(start, _, _)| *start)
        {
            rendered.sql.push_str(&text[cursor..start]);
            let output_start: usize = rendered.sql.len();
            rendered.sql.push_str(value);
            rendered.splices.push(TextSplice {
                source: (start, start + reference.len()),
                output: (output_start, rendered.sql.len()),
            });
            cursor = start + reference.len();
        }
        rendered.sql.push_str(&text[cursor..]);
        Ok(rendered)
    }

    fn constant_shape(
        &self,
        _scope: &SqlMacroScope,
        name: &str,
    ) -> Result<ConstantShape, ExpansionFailure<String>> {
        Ok(if name == "supported_countries" {
            ConstantShape::Collection
        } else {
            ConstantShape::Scalar
        })
    }

    fn render_collection(&self, text: &str) -> Result<String, ExpansionFailure<String>> {
        let inner: &str = text
            .trim()
            .trim_start_matches(['[', '{'])
            .trim_end_matches([']', '}']);
        let items: Vec<String> = inner
            .split(',')
            .map(|item| format!("'{}'", item.trim().trim_matches('"')))
            .collect();
        Ok(format!("({})", items.join(", ")))
    }

    fn adapter(&self) -> &str {
        self.adapter
    }
}

/// `sql` expanded with `macros` for `adapter`, or the SQL macro error it raised.
pub(super) fn expanded_for(
    adapter: &'static str,
    macros: &[(&str, &str)],
    sql: &str,
) -> Result<String, SqlMacroError> {
    expanded_text_for(adapter, macros, sql).map(|expanded| expanded.sql)
}

/// `sql` expanded with `macros` for `adapter` with its typed calls, or the error it raised.
pub(super) fn expanded_text_for(
    adapter: &'static str,
    macros: &[(&str, &str)],
    sql: &str,
) -> Result<ExpandedText, SqlMacroError> {
    let host: FakeHost = FakeHost::new(macros, adapter);
    let context = TextContext {
        text: sql,
        scope: &SqlMacroScope::Caller,
        stack: &[],
        owner: "model:orders_clean",
    };
    expand_text(&host, &context).map_err(macro_error)
}

/// The SQL macro error of a failed expansion; the fake host itself never fails.
fn macro_error(failure: ExpansionFailure<String>) -> SqlMacroError {
    let ExpansionFailure::Macro(error) = failure else {
        panic!("the fake host failed");
    };
    error
}

/// Each name as an owned string.
pub(super) fn owned(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

impl sqlbuild_render::macro_arguments::types::ArgumentHost for AsciiArguments {
    fn character_named(&self, _name: &str) -> Option<String> {
        None
    }

    fn identifier(&self, _text: &str) -> Option<String> {
        None
    }
}

/// Each call as `macro returns: text`.
pub(super) fn shown_typed_calls(sql: &str, calls: &[TypedCall]) -> Vec<String> {
    calls
        .iter()
        .map(|call| {
            format!(
                "{} {}: {}",
                call.macro_name,
                call.returns.as_deref().unwrap_or("-"),
                &sql[call.start..call.end]
            )
        })
        .collect()
}

/// An untyped `shout` call of each text in `sql`.
pub(super) fn untyped_calls(sql: &str, texts: &[&str]) -> Vec<TypedCall> {
    texts
        .iter()
        .map(|text| {
            let start: usize = sql.find(text).unwrap_or_default();
            TypedCall {
                returns_like: None,
                start,
                end: start + text.len(),
                macro_name: "shout".to_owned(),
                returns: None,
            }
        })
        .collect()
}

pub(super) fn escape_prefix_rules() -> sqlbuild_sqltext::sql_scan::models::LexicalSyntax {
    sqlbuild_sqltext::sql_scan::models::LexicalSyntax {
        escape_string_prefix: true,
        line_comment_prefixes: vec!["--".to_owned()],
        ..sqlbuild_sqltext::sql_scan::models::LexicalSyntax::default()
    }
}

pub(super) fn backslash_rules() -> sqlbuild_sqltext::sql_scan::models::LexicalSyntax {
    sqlbuild_sqltext::sql_scan::models::LexicalSyntax {
        backslash_escape_quotes: vec!["'".to_owned()],
        line_comment_prefixes: vec!["--".to_owned(), "//".to_owned()],
        ..sqlbuild_sqltext::sql_scan::models::LexicalSyntax::default()
    }
}

pub(super) fn raw_prefix_rules() -> sqlbuild_sqltext::sql_scan::models::LexicalSyntax {
    sqlbuild_sqltext::sql_scan::models::LexicalSyntax {
        backslash_escape_quotes: vec!["'".to_owned(), "\"".to_owned()],
        raw_string_prefix: true,
        line_comment_prefixes: vec!["--".to_owned(), "#".to_owned()],
        ..sqlbuild_sqltext::sql_scan::models::LexicalSyntax::default()
    }
}
