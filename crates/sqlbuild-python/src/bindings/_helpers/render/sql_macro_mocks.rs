//! SQL test `__macro__<name>` mocks of SQL macros, applied to a model's declaration-pass SQL.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};

use sqlbuild_python_base::records::main::fields::field::field as native_field;

const SQL_LANGUAGE: &str = "sql";
const SQL_CALL_PREFIX: &str = "@sql.";
const LIST_SEPARATOR: char = ',';

/// `macro_source_sql` with each call of a mocked SQL macro the model makes replaced by its mock.
pub(crate) fn with_sql_macro_mocks(
    model_input: &Bound<'_, PyAny>,
    macro_source_sql: String,
    (mocks, loaded_macros): (&Bound<'_, PyDict>, &Bound<'_, PyAny>),
) -> PyResult<String> {
    let mut mocked: Vec<(String, String)> = Vec::new();
    for (name, value) in mocks.iter() {
        let name: String = name.extract()?;
        let Ok(found) = loaded_macros.get_item(&name) else {
            continue;
        };
        if native_field(&found, "language")?.extract::<String>()? == SQL_LANGUAGE {
            mocked.push((name, value.extract()?));
        }
    }
    if mocked.is_empty() {
        return Ok(macro_source_sql);
    }
    let expansion: Bound<'_, PyAny> = native_field(model_input, "sql_expansion")?;
    if expansion.is_none() {
        return Ok(macro_source_sql);
    }
    let authored: Vec<char> = native_field(&expansion, "authored_sql")?
        .extract::<String>()?
        .chars()
        .collect();
    let Some(declaration_pass) = native_field(&expansion, "passes")?.try_iter()?.next() else {
        return Ok(macro_source_sql);
    };
    let mut replacements: Vec<(usize, usize, &str)> = Vec::new();
    for span in declaration_pass?.try_iter()? {
        let span: Bound<'_, PyAny> = span?;
        let source: (usize, usize) = (
            native_field(&span, "source_start")?.extract()?,
            native_field(&span, "source_end")?.extract()?,
        );
        let Some(called) = authored.get(source.0..source.1).and_then(called_sql_macro) else {
            continue;
        };
        if let Some((_, value)) = mocked.iter().find(|(name, _)| *name == called) {
            replacements.push((
                native_field(&span, "output_start")?.extract()?,
                native_field(&span, "output_end")?.extract()?,
                value,
            ));
        }
    }
    let mut text: Vec<char> = macro_source_sql.chars().collect();
    replacements.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
    for (start, end, value) in replacements {
        if start <= end && end <= text.len() {
            let mocked: String = with_separators(&text[start..end], value);
            let _ = text.splice(start..end, mocked.chars());
        }
    }
    Ok(text.into_iter().collect())
}

/// The macro name of authored text that is one `@sql.name(...)` call.
fn called_sql_macro(text: &[char]) -> Option<String> {
    let text: String = text.iter().collect();
    let rest: &str = text.strip_prefix(SQL_CALL_PREFIX)?;
    let name: &str = &rest[..rest.find('(')?];
    Some(name.trim().to_owned())
}

/// The mock value with the list separators the replaced expansion wrote around itself.
fn with_separators(expanded: &[char], value: &str) -> String {
    let leading: bool = expanded.starts_with(&[LIST_SEPARATOR, ' ']);
    let trailing: bool = expanded.last() == Some(&LIST_SEPARATOR);
    format!(
        "{}{value}{}",
        if leading { ", " } else { "" },
        if trailing { "," } else { "" }
    )
}
