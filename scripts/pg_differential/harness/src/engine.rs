//! The adapter dialect of a case: its Polyglot and SQLBuild names, SQLQuack's parser and profile.

use sqlbuild_analysis::planner_migrations::models::ParsedStatement;
use sqlquack_semantics::dialects::constants::{DUCKDB, POSTGRES, SNOWFLAKE};
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_syntax::{
    CallMode, Dialect, Expression, ParseOptions, TokenKind, parse_statements_with,
};
use sqlquack_syntax_postgres::scan::main::scan::scan;
use sqlquack_syntax_postgres::scan::models::TokenKind as PostgresTokenKind;

/// One of the adapters the harness compares.
#[derive(Clone, Copy, Debug)]
pub enum Engine {
    Postgres,
    Snowflake,
    DuckDb,
}

/// One statement as SQLQuack parsed it, or why it is not measured.
pub enum Parsed {
    Statement(Expression),
    /// `parse_error`, `opaque` (a construct kept as a command) or another reason, with detail.
    Unmeasured(&'static str, String),
}

impl Engine {
    /// The engine for a capture's Polyglot dialect name.
    pub fn from_polyglot(dialect: &str) -> Result<Self, String> {
        match dialect {
            "postgresql" => Ok(Self::Postgres),
            "snowflake" => Ok(Self::Snowflake),
            "duckdb" => Ok(Self::DuckDb),
            other => Err(format!("unsupported dialect {other}")),
        }
    }

    /// The name Polyglot validates as.
    pub fn polyglot_name(self) -> &'static str {
        match self {
            Self::Postgres => "postgresql",
            Self::Snowflake => "snowflake",
            Self::DuckDb => "duckdb",
        }
    }

    /// The adapter's analysis dialect name in SQLBuild.
    pub fn analysis_name(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Snowflake => "snowflake",
            Self::DuckDb => "duckdb",
        }
    }

    pub fn profile(self) -> &'static dyn DialectProfile {
        match self {
            Self::Postgres => &POSTGRES,
            Self::Snowflake => &SNOWFLAKE,
            Self::DuckDb => &DUCKDB,
        }
    }

    /// Whether a catalog column name only binds exactly, as if quoted: on Postgres any name that
    /// is not all lower case, on Snowflake a mixed-case name, never on DuckDB.
    pub fn exact_name(self, name: &str) -> bool {
        match self {
            Self::Postgres => name != name.to_lowercase(),
            Self::Snowflake => name != name.to_lowercase() && name != name.to_uppercase(),
            Self::DuckDb => false,
        }
    }

    /// The single statement of `sql`: Postgres through the 18.6 parser, the others through
    /// sqlquack-syntax with located calls, as `query_lineage_and_findings` parses them.
    pub fn parse(self, sql: &str) -> Parsed {
        let dialect: Dialect = match self {
            Self::Postgres => return postgres_statement(sql),
            Self::Snowflake => Dialect::Snowflake,
            Self::DuckDb => Dialect::DuckDB,
        };
        match parse_statements_with(
            sql,
            dialect,
            &ParseOptions::default(),
            CallMode::FaithfulLocated,
        ) {
            Ok(mut statements) if statements.len() == 1 => Parsed::Statement(statements.remove(0)),
            Ok(statements) => {
                Parsed::Unmeasured("parse_error", format!("{} statements", statements.len()))
            }
            Err(error) => Parsed::Unmeasured("parse_error", error.to_string()),
        }
    }

    /// The statement and its token spans in characters, as the migration fingerprint reads them.
    pub fn fingerprint_statement(self, sql: &str) -> Result<Option<ParsedStatement>, String> {
        let Parsed::Statement(statement) = self.parse(sql) else {
            return Ok(None);
        };
        let tokens: Vec<std::ops::Range<usize>> = match self {
            Self::Postgres => {
                let tokens = scan(sql);
                if tokens
                    .iter()
                    .any(|token| matches!(token.kind, PostgresTokenKind::Fault(_)))
                {
                    return Ok(None);
                }
                tokens
                    .iter()
                    .map(|token| token.span.start..token.span.end)
                    .collect()
            }
            Self::Snowflake | Self::DuckDb => {
                let dialect = if matches!(self, Self::Snowflake) {
                    Dialect::Snowflake
                } else {
                    Dialect::DuckDB
                };
                let Ok(tokens) = sqlquack_syntax::tokenize(sql, dialect) else {
                    return Ok(None);
                };
                let characters: Vec<usize> = character_offsets(sql);
                tokens
                    .iter()
                    .filter(|token| matches!(token.kind, TokenKind::Significant(_)))
                    .map(|token| characters[token.start]..characters[token.end])
                    .collect()
            }
        };
        Ok(Some(ParsedStatement {
            tree: serde_json::to_value(&statement).map_err(|error| error.to_string())?,
            tokens,
        }))
    }
}

fn postgres_statement(sql: &str) -> Parsed {
    let parsed = match sqlquack_syntax_postgres::parse(sql) {
        Ok(parsed) => parsed,
        Err(error) => return Parsed::Unmeasured("parse_error", format!("{error:?}")),
    };
    if !parsed.gaps.is_empty() {
        let names: Vec<&str> = parsed.gaps.iter().map(|gap| gap.construct).collect();
        return Parsed::Unmeasured("opaque", names.join(","));
    }
    let mut statements = parsed.statements;
    if statements.len() != 1 {
        return Parsed::Unmeasured("parse_error", format!("{} statements", statements.len()));
    }
    Parsed::Statement(statements.remove(0))
}

/// The character offset of every byte offset of `sql`, including its end.
fn character_offsets(sql: &str) -> Vec<usize> {
    let mut offsets: Vec<usize> = vec![0; sql.len() + 1];
    let mut characters: usize = 0;
    for (byte, character) in sql.char_indices() {
        for offset in &mut offsets[byte..byte + character.len_utf8()] {
            *offset = characters;
        }
        characters += 1;
    }
    offsets[sql.len()] = characters;
    offsets
}
