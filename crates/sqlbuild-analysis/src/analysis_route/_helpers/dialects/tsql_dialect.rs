//! SQL Server's native module: SQLQuack's T-SQL parser and its sqlquack-semantics profile.

use sqlquack_semantics::dialects::constants::SQL_SERVER;
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_syntax::{DataType, Expression, Gap, Parsed};
use sqlquack_syntax_sqlserver::parse_strict;

use crate::analysis_route::constants::{
    LOGICAL_TYPES, TSQL_BUILT_IN_TYPES, TSQL_DIALECT, TSQL_STUB_PREFIX, TSQL_TYPE_ALIASES,
    TSQL_TYPE_SYNONYMS,
};
use crate::analysis_route::models::{ParserGap, TsqlDialect};
use crate::analysis_route::types::NativeDialect;
use crate::type_system::main::normalize_text_type::normalize_text_type;
use crate::type_system::models::{NormalizedType, TypeDialect, TypeFamily};

impl NativeDialect for TsqlDialect {
    fn name(&self) -> &'static str {
        TSQL_DIALECT
    }

    fn stub_prefix(&self) -> &'static str {
        TSQL_STUB_PREFIX
    }

    /// Every statement with its gaps; an unknown one-part function (195) is a syntax error, as the engine raises it.
    fn parse(&self, sql: &str) -> Result<Parsed, ParserGap> {
        let parsed = parse_strict(sql).map_err(|failure| {
            let error = failure.parse_error();
            ParserGap {
                construct: error.message.clone(),
                message: error.message.clone(),
                offset: error.position,
                syntax: error.syntax,
            }
        })?;
        Ok(Parsed {
            statements: parsed.statements,
            gaps: parsed
                .gaps
                .into_iter()
                .map(|gap| Gap {
                    construct: gap.construct,
                    start: gap.start,
                    end: gap.end,
                })
                .collect(),
        })
    }

    fn profile(&self) -> &'static dyn DialectProfile {
        &SQL_SERVER
    }

    /// The type spelt with Polyglot's T-SQL canonical names; `None` for a type neither SQL Server nor SQLBuild knows.
    fn normalize_type(&self, spelling: &str) -> Option<NormalizedType> {
        let written: String = spelling
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
            .to_ascii_uppercase();
        let (base, arguments): (&str, String) = match written.find('(') {
            Some(open) => (written[..open].trim(), written[open..].replace(' ', "")),
            None => (written.as_str(), String::new()),
        };
        if let Some(direct) = canonical_type(base, &arguments) {
            return Some(direct);
        }
        if !LOGICAL_TYPES.contains(&base) && !built_in_type(self, &written) {
            return None;
        }
        let aliased: String = match type_alias(base) {
            Some(alias) if alias.contains('(') => alias.to_owned(),
            Some(alias) => format!("{alias}{arguments}"),
            None => written.clone(),
        };
        Some(normalize_text_type(&aliased, Some(TypeDialect::Tsql)))
    }
}

/// Whether SQL Server reads `spelling` as one of its built-in types.
fn built_in_type(dialect: &TsqlDialect, spelling: &str) -> bool {
    let Ok(parsed) = dialect.parse(&format!("SELECT CAST(NULL AS {spelling})")) else {
        return false;
    };
    let target: Option<&DataType> = match (parsed.gaps.is_empty(), parsed.statements.as_slice()) {
        (true, [Expression::Select(select)]) => match select.expressions.as_slice() {
            [Expression::Cast(cast)] => Some(&cast.to),
            _ => None,
        },
        _ => None,
    };
    match target {
        Some(DataType::Custom { name }) => {
            let upper: String = name.to_ascii_uppercase();
            let base: &str = upper.split('(').next().unwrap_or_default().trim();
            TSQL_BUILT_IN_TYPES.contains(&base) || TSQL_TYPE_SYNONYMS.contains(&base)
        }
        Some(_) => true,
        None => false,
    }
}

/// The types whose canonical name or family the text fallback cannot give, built directly.
fn canonical_type(base: &str, arguments: &str) -> Option<NormalizedType> {
    let (name, family): (String, TypeFamily) = match base {
        "BOOLEAN" => ("BIT".to_owned(), TypeFamily::Boolean),
        "TIMESTAMP" => ("DATETIME2".to_owned(), TypeFamily::Timestamp),
        "TEXT" | "NTEXT" | "STRING" => ("VARCHAR(MAX)".to_owned(), TypeFamily::String),
        "FLOAT" if !arguments.is_empty() => (format!("FLOAT{arguments}"), TypeFamily::Float),
        _ => return None,
    };
    Some(NormalizedType {
        normalized_name: name,
        family,
        precision: None,
        scale: None,
        length: None,
    })
}

/// The canonical name Polyglot gives an alias, before the text fallback reads it.
fn type_alias(base: &str) -> Option<&'static str> {
    TSQL_TYPE_ALIASES
        .iter()
        .find(|(alias, _)| *alias == base)
        .map(|(_, canonical)| *canonical)
}
