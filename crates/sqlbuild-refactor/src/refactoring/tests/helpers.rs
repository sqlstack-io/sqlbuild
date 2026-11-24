use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use sqlbuild_sqltext::sql_scan::main::quote_policy::quote_policy;

use crate::refactoring::_helpers::edits::header_edits::{
    add_column_entry_edit, column_config_edits, column_entry_edits, consumer_column_header_edits,
    header_tokens, insert_header_entry_edit, model_name_header_edits, schema_column_edits,
    schema_model_name_edits,
};
use crate::refactoring::_helpers::edits::text_edits::{apply_text_edits, file_changes, text_edit};
use crate::refactoring::_helpers::edits::yaml_edits::{yaml_column_edits, yaml_model_edits};
use crate::refactoring::_helpers::files::paths::stem;
use crate::refactoring::_helpers::files::workspace::commit_changes;
use crate::refactoring::_helpers::planning::column_planning::{
    column_rename_context, header_parts,
};
use crate::refactoring::_helpers::planning::column_references::{
    BodyContext, BodyMapping, ColumnQuery, ResourceColumns, analyze_column, consumer_edits,
};
use crate::refactoring::_helpers::planning::declaration_moves::{ModelMove, declaration_moves};
use crate::refactoring::_helpers::planning::model_planning::{model_parts, model_target};
use crate::refactoring::_helpers::scanning::chars::chars;
use crate::refactoring::_helpers::scanning::scan_context::ScanContext;
use crate::refactoring::_helpers::scanning::sql_sites::{ModelBody, analysis_sql, resource_sites};
use crate::refactoring::errors::{RefactorError, RefactorErrorKind};
use crate::refactoring::models::{
    AuthoredFile, DeclarationMoves, DeclarationPlacement, DestinationFacts, DiscoveredFile,
    EditKind, ExpansionSpan, FileChange, ModelFacts, PlacedDeclaration, RefactorFacts,
    RefactorOperation, RefactorRequest, RelocatedDeclaration, SqlFileRole, TextEdit, UnitTestFacts,
};
use crate::refactoring::tests::test_types::{ExpectedSpans, PlannedMoves, Spans};

pub(super) fn context(dialect: &str) -> ScanContext {
    ScanContext {
        backtick_identifiers: quote_policy(dialect).backtick_identifiers,
    }
}

/// The text after applying edits.
pub(super) fn applied(text: &str, edits: &[TextEdit]) -> Result<String, RefactorError> {
    Ok(apply_text_edits(&chars(text), edits)?.into_iter().collect())
}

/// Each edit as `line:column kind before -> after`, the way the output tree shows it.
pub(super) fn described(edits: &[TextEdit]) -> Vec<String> {
    edits
        .iter()
        .map(|edit| {
            format!(
                "{}:{} {} {:?} -> {:?}",
                edit.line,
                edit.column,
                edit.kind.value(),
                edit.before,
                edit.after
            )
        })
        .collect()
}

/// The texts of spans of `text`.
pub(super) fn span_texts(text: &str, spans: &[(usize, usize)]) -> Vec<String> {
    let characters: Vec<char> = chars(text);
    spans
        .iter()
        .map(|(start, end)| characters[*start..*end].iter().collect())
        .collect()
}

/// Reference edits of `text` from `(start, end, replacement)` triples.
pub(super) fn reference_edits(text: &str, edits: &[(usize, usize, &'static str)]) -> Vec<TextEdit> {
    let characters: Vec<char> = chars(text);
    edits
        .iter()
        .map(|(start, end, replacement)| {
            text_edit(
                &characters,
                (*start, *end),
                (*replacement).to_owned(),
                EditKind::Reference,
            )
        })
        .collect()
}

pub(super) fn owned_sites(sites: &[(usize, usize, &str)]) -> Vec<(usize, usize, String)> {
    sites
        .iter()
        .map(|(start, end, name)| (*start, *end, (*name).to_owned()))
        .collect()
}

/// `(path, original path, edit count)` of each grouped change.
pub(super) fn grouped_changes(
    edited_paths: &[&str],
    moves: &[(&str, &str)],
) -> Vec<(String, String, usize)> {
    let edit: TextEdit = reference_edits("abc", &[(0, 1, "x")]).remove(0);
    let edits: Vec<(String, TextEdit)> = edited_paths
        .iter()
        .map(|path| ((*path).to_owned(), edit.clone()))
        .collect();
    let moved: Vec<(String, String)> = moves
        .iter()
        .map(|(from, to)| ((*from).to_owned(), (*to).to_owned()))
        .collect();
    file_changes(edits, &moved)
        .into_iter()
        .map(|change| (change.path, change.original_path, change.edits.len()))
        .collect()
}

pub(super) fn owned_changes(changes: &[(&str, &str, usize)]) -> Vec<(String, String, usize)> {
    changes
        .iter()
        .map(|(path, original, count)| ((*path).to_owned(), (*original).to_owned(), *count))
        .collect()
}

/// Each resource site as `kind name call-text name-text`.
pub(super) fn resource_site_lines(sql: &str) -> Vec<String> {
    resource_sites(&chars(sql), &context("duckdb"))
        .into_iter()
        .map(|site| {
            let texts: Vec<String> = span_texts(
                sql,
                &[(site.start, site.end), (site.name_start, site.name_end)],
            );
            format!("{} {} {} {}", site.kind, site.name, texts[0], texts[1])
        })
        .collect()
}

/// The analysis SQL and each table as `kind name placeholder,placeholder`.
pub(super) fn analysis_lines(sql: &str) -> (String, Vec<String>) {
    let analysis = analysis_sql(&chars(sql), &context("duckdb"));
    let tables: Vec<String> = analysis
        .tables
        .iter()
        .map(|((kind, name), placeholders)| {
            let joined: Vec<String> = placeholders.iter().cloned().collect();
            format!("{kind} {name} {}", joined.join(","))
        })
        .collect();
    (analysis.sql, tables)
}

/// A body at offset 10 whose one expansion turns `@py.m(x)` into a five-character call.
pub(super) fn expansion_body() -> ModelBody {
    ModelBody {
        body_start: 10,
        compiled_sql: "SELECT a + b FROM t".to_owned(),
        passes: vec![vec![ExpansionSpan {
            source_start: 7,
            source_end: 11,
            output_start: 7,
            output_end: 12,
        }]],
    }
}

pub(super) fn consumer_model_edits(contents: &'static str) -> Result<Vec<TextEdit>, RefactorError> {
    model_name_header_edits(contents, &chars(contents), "stg_orders", "stg_order_lines")
}

pub(super) fn owner_column_edits(contents: &'static str) -> Result<Vec<TextEdit>, RefactorError> {
    let text: Vec<char> = chars(contents);
    let tokens = header_tokens(contents, &text)?.unwrap_or_default();
    let mut edits: Vec<TextEdit> = column_config_edits(&text, &tokens, "order_id", "order_key");
    edits.extend(
        column_entry_edits(&text, &tokens, ("order_id", "order_key"), true).unwrap_or_default(),
    );
    Ok(edits)
}

pub(super) fn schema_model_edits(contents: &'static str) -> Result<Vec<TextEdit>, RefactorError> {
    schema_model_name_edits(
        contents,
        &chars(contents),
        ("stg_orders", "stg_order_lines"),
    )
}

pub(super) fn schema_field_edits(contents: &'static str) -> Result<Vec<TextEdit>, RefactorError> {
    schema_column_edits(
        contents,
        &chars(contents),
        ("stg_orders", ("order_id", "order_key")),
    )
}

/// The contents with a leading `migrate_from old_orders` entry, or `None` without a header.
pub(super) fn inserted_entry(contents: &str) -> Result<Option<String>, RefactorError> {
    insert_header_entry_edit(
        contents,
        &chars(contents),
        "migrate_from old_orders",
        "migrate_from old_orders",
    )
    .map(|edit| applied(contents, &[edit]))
    .transpose()
}

fn yaml_file(contents: &str) -> Vec<DiscoveredFile> {
    vec![DiscoveredFile {
        path: "sql/sources/raw.yml".to_owned(),
        contents: contents.to_owned(),
    }]
}

fn only_edits(edits: Vec<(String, TextEdit)>) -> Vec<TextEdit> {
    edits.into_iter().map(|(_, edit)| edit).collect()
}

pub(super) fn yaml_reference_edits(contents: &'static str) -> Result<Vec<TextEdit>, RefactorError> {
    Ok(only_edits(yaml_model_edits(
        &yaml_file(contents),
        "stg_orders",
        "stg_order_lines",
    )?))
}

pub(super) fn yaml_field_edits(contents: &'static str) -> Result<Vec<TextEdit>, RefactorError> {
    Ok(only_edits(yaml_column_edits(
        &yaml_file(contents),
        "stg_orders",
        "order_id",
        "order_key",
    )?))
}

/// `(start, end, before)` of renaming model `stg_orders`, then column `order_id` of it.
pub(super) fn yaml_spans(contents: &str) -> Result<(Spans, Spans), RefactorError> {
    let models: Vec<(usize, usize, String)> =
        spans(yaml_model_edits(&yaml_file(contents), "stg_orders", "x")?);
    let columns: Vec<(usize, usize, String)> = spans(yaml_column_edits(
        &yaml_file(contents),
        "stg_orders",
        "order_id",
        "k",
    )?);
    Ok((models, columns))
}

fn spans(edits: Vec<(String, TextEdit)>) -> Vec<(usize, usize, String)> {
    edits
        .into_iter()
        .map(|(_, edit)| (edit.start, edit.end, edit.before))
        .collect()
}

pub(super) fn owned_span_pair(spans: (ExpectedSpans, ExpectedSpans)) -> (Spans, Spans) {
    (owned_sites(spans.0), owned_sites(spans.1))
}

pub(super) fn model(name: &str, materialized: Option<&str>, schema: &str) -> ModelFacts {
    ModelFacts {
        name: name.to_owned(),
        path: format!("sql/models/{name}.sql"),
        deps: Vec::new(),
        query_sql: "SELECT 1".to_owned(),
        authored_query_sql: "SELECT 1".to_owned(),
        materialized: materialized.map(str::to_owned),
        declares_migrate_from: false,
        migrate_from_set: false,
        destination: DestinationFacts {
            database: Some("warehouse".to_owned()),
            schema: Some(schema.to_owned()),
            name: name.to_owned(),
        },
        inferred_columns: Vec::new(),
        schema_columns: None,
        expansion: None,
    }
}

/// `model` placed in another database.
pub(super) fn model_in_database(name: &str, database: &str) -> ModelFacts {
    let mut moved: ModelFacts = model(name, Some("table"), "analytics");
    moved.destination.database = Some(database.to_owned());
    moved
}

pub(super) fn facts(project_dir: &str, models: Vec<ModelFacts>) -> RefactorFacts {
    RefactorFacts {
        project_dir: project_dir.to_owned(),
        dialect: "duckdb".to_owned(),
        models,
        sources: Vec::new(),
        seeds: Vec::new(),
        model_files: Vec::new(),
        authored_files: Vec::new(),
        yaml_files: Vec::new(),
        unit_tests: Vec::new(),
        python_locations: Vec::new(),
    }
}

/// A request on model `orders`.
pub(super) fn request(
    operation: RefactorOperation,
    new_name: &str,
    destination: Option<&str>,
) -> RefactorRequest {
    RefactorRequest {
        operation,
        model_name: "orders".to_owned(),
        new_name: new_name.to_owned(),
        column_name: None,
        destination: destination.map(str::to_owned),
        cascade: false,
    }
}

/// Each blocking item as `path: reason`.
pub(super) fn blocking_reasons(
    facts: &RefactorFacts,
    request: &RefactorRequest,
) -> Result<Vec<String>, RefactorError> {
    let target = model_target(facts, request)?;
    let context = ScanContext {
        backtick_identifiers: false,
    };
    let host = |_: &[(String, String)]| {
        Ok(DeclarationPlacement {
            relocated: Some(Vec::new()),
            ..DeclarationPlacement::default()
        })
    };
    let test_host = |_: &str, _: &str| Ok(Some(Vec::new()));
    let parts = model_parts(facts, &target, &context, (&host, &test_host))?;
    Ok(parts
        .blocking
        .into_iter()
        .map(|item| format!("{}: {}", item.path, item.reason))
        .collect())
}

/// The `(code, message)` a model request is refused with.
pub(super) fn refused_target(
    facts: &RefactorFacts,
    request: &RefactorRequest,
) -> Option<(String, String)> {
    model_target(facts, request)
        .err()
        .map(|error| (error.code, error.message))
}

/// An edit replacing all of `text`.
pub(super) fn replace_all(text: &str, replacement: &str) -> TextEdit {
    TextEdit {
        start: 0,
        end: text.chars().count(),
        replacement: replacement.to_owned(),
        kind: EditKind::Reference,
        line: 1,
        column: 1,
        before: text.to_owned(),
        after: replacement.to_owned(),
    }
}

pub(super) fn change(path: &str, original_path: &str, edits: Vec<TextEdit>) -> FileChange {
    FileChange {
        path: path.to_owned(),
        original_path: original_path.to_owned(),
        edits,
    }
}

pub(super) fn owned_pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(left, right)| ((*left).to_owned(), (*right).to_owned()))
        .collect()
}

/// Write `files` below `root`.
pub(super) fn write_files(root: &Path, files: &[(&str, &str)]) -> Result<(), RefactorError> {
    files
        .iter()
        .try_for_each(|(path, contents)| {
            let target = root.join(path);
            fs::create_dir_all(target.parent().unwrap_or(root))
                .and_then(|()| fs::write(target, contents))
        })
        .map_err(|error| RefactorError::value(error.to_string()))
}

/// Every file named in `files` as it is on disk now.
pub(super) fn files_on_disk(root: &Path, files: &[(&str, &str)]) -> Vec<(String, String)> {
    files
        .iter()
        .map(|(path, _)| {
            (
                (*path).to_owned(),
                fs::read_to_string(root.join(path)).unwrap_or_default(),
            )
        })
        .collect()
}

/// `(kind, code, whether the message starts with prefix)` of a failed commit.
pub(super) fn commit_failure(
    root: &Path,
    originals: &[(&str, &str)],
    changes: &[FileChange],
    prefix: &str,
) -> Option<(RefactorErrorKind, String, bool)> {
    commit_changes(root, &owned_pairs(originals), changes)
        .err()
        .map(|error| (error.kind, error.code, error.message.starts_with(prefix)))
}

/// A scope-index declaration `kind:name` at `path`, line 1.
pub(super) fn placed(label: &str, path: &str) -> PlacedDeclaration {
    PlacedDeclaration {
        key: label.to_owned(),
        label: label.to_owned(),
        path: path.to_owned(),
        line: Some(1),
        column: Some(1),
    }
}

pub(super) fn relocated(label: &str, path: &str) -> RelocatedDeclaration {
    RelocatedDeclaration {
        key: label.to_owned(),
        path: path.to_owned(),
        requires_group: false,
    }
}

/// The moves and `path:line reason` blockers of moving models together with a fixed placement.
pub(super) fn planned_declaration_moves(
    root: &Path,
    paths: &[(&str, &str)],
    placement: &DeclarationPlacement,
) -> Result<PlannedMoves, RefactorError> {
    let host = |_: &[(String, String)]| Ok(placement.clone());
    let models: Vec<ModelMove<'_>> = paths
        .iter()
        .map(|(source, destination)| ModelMove {
            model: stem(source),
            source,
            destination,
        })
        .collect();
    let moves: DeclarationMoves = declaration_moves(root, &models, &BTreeSet::new(), &host)?;
    let blocking: Vec<String> = moves
        .blocking
        .iter()
        .map(|item| {
            format!(
                "{}:{} {}",
                item.path,
                item.line.unwrap_or_default(),
                item.reason
            )
        })
        .collect();
    Ok((moves.moves, blocking))
}

/// Rename `fact_orders.amount` in one consumer body: edited SQL, pass-through, manual reasons.
pub(super) fn plan_consumer(
    sql: &str,
    cascade: bool,
) -> Result<(String, bool, Vec<String>), RefactorError> {
    let text: Vec<char> = chars(sql);
    let analysis = analysis_sql(&text, &context("duckdb"));
    let columns: ResourceColumns = vec![
        (
            ("ref".to_owned(), "fact_orders".to_owned()),
            vec![
                "order_id".to_owned(),
                "customer_id".to_owned(),
                "amount".to_owned(),
            ],
        ),
        (
            ("ref".to_owned(), "customers".to_owned()),
            vec!["customer_id".to_owned(), "amount".to_owned()],
        ),
    ];
    let facts = analyze_column(
        &analysis,
        "duckdb",
        &columns,
        &ColumnQuery {
            column: "amount",
            target_tables: analysis.placeholders("ref", "fact_orders"),
            target_ctes: BTreeSet::new(),
            output_scopes: BTreeSet::new(),
        },
    )?;
    let body = BodyContext {
        path: "sql/models/consumer.sql",
        contents: &text,
        mapping: BodyMapping::Authored(0),
        fallback_offset: 0,
    };
    let result = consumer_edits(&facts, &body, ("amount", "revenue"), (cascade, cascade));
    let reasons: Vec<String> = result.manual.into_iter().map(|item| item.reason).collect();
    Ok((applied(sql, &result.edits)?, result.passes_through, reasons))
}

/// Rename model `fact_orders` to `order_facts` in source and seed YAML.
pub(super) fn yaml_fact_model_edits(
    contents: &'static str,
) -> Result<Vec<TextEdit>, RefactorError> {
    Ok(only_edits(yaml_model_edits(
        &yaml_file(contents),
        "fact_orders",
        "order_facts",
    )?))
}

/// Rename column `fact_orders.amount` to `revenue` in source and seed YAML.
pub(super) fn yaml_fact_column_edits(
    contents: &'static str,
) -> Result<Vec<TextEdit>, RefactorError> {
    Ok(only_edits(yaml_column_edits(
        &yaml_file(contents),
        "fact_orders",
        "amount",
        "revenue",
    )?))
}

pub(super) fn header_fact_model_edits(
    contents: &'static str,
) -> Result<Vec<TextEdit>, RefactorError> {
    model_name_header_edits(contents, &chars(contents), "fact_orders", "order_facts")
}

pub(super) fn header_fact_column_edits(
    contents: &'static str,
) -> Result<Vec<TextEdit>, RefactorError> {
    consumer_column_header_edits(
        contents,
        &chars(contents),
        "fact_orders",
        ("amount", "revenue"),
    )
}

pub(super) fn schema_fact_model_edits(
    contents: &'static str,
) -> Result<Vec<TextEdit>, RefactorError> {
    schema_model_name_edits(contents, &chars(contents), ("fact_orders", "order_facts"))
}

pub(super) fn schema_fact_column_edits(
    contents: &'static str,
) -> Result<Vec<TextEdit>, RefactorError> {
    schema_column_edits(
        contents,
        &chars(contents),
        ("fact_orders", ("amount", "revenue")),
    )
}

/// The owner header edits renaming `amount` to `revenue` with a column migration.
pub(super) fn migrated_column_edits(
    contents: &'static str,
) -> Result<Vec<TextEdit>, RefactorError> {
    let text: Vec<char> = chars(contents);
    let tokens = header_tokens(contents, &text)?.unwrap_or_default();
    let entries: Option<Vec<TextEdit>> =
        column_entry_edits(&text, &tokens, ("amount", "revenue"), true);
    let added: Vec<TextEdit> = entries.clone().unwrap_or_else(|| {
        add_column_entry_edit(contents, &text, &tokens, ("amount", "revenue"))
            .into_iter()
            .collect()
    });
    let mut edits: Vec<TextEdit> = column_config_edits(&text, &tokens, "amount", "revenue");
    edits.extend(added);
    Ok(edits)
}

/// The header entry `migrate_from fact_orders` inserted at the top of the header.
pub(super) fn fact_orders_migration_entry(
    contents: &'static str,
) -> Result<Vec<TextEdit>, RefactorError> {
    Ok(insert_header_entry_edit(
        contents,
        &chars(contents),
        "migrate_from fact_orders",
        "migrate_from fact_orders",
    )
    .into_iter()
    .collect())
}

/// Owned `(name, type)` physical columns.
pub(crate) fn physical_columns(columns: &[(&str, &str)]) -> Vec<(String, String)> {
    columns
        .iter()
        .map(|(name, column_type)| ((*name).to_owned(), (*column_type).to_owned()))
        .collect()
}

/// Header edits and manual locations of renaming `orders.is_flagged` to `is_marked`.
pub(super) fn owner_header_parts(
    contents: &str,
) -> Result<(Vec<String>, Vec<String>), RefactorError> {
    let mut orders = model("orders", Some("table"), "analytics");
    orders.inferred_columns = vec!["order_id".to_owned(), "is_flagged".to_owned()];
    let mut vip_flags = model("vip_flags", Some("table"), "analytics");
    vip_flags.inferred_columns = vec!["is_flagged".to_owned()];
    let unknown_flags = model("unknown_flags", Some("table"), "analytics");
    let mut project_facts = facts(
        "/nonexistent/project",
        vec![orders.clone(), vip_flags, unknown_flags],
    );
    project_facts.model_files = vec![DiscoveredFile {
        path: orders.path.clone(),
        contents: contents.to_owned(),
    }];
    project_facts.authored_files = [
        (
            "flag_check",
            "SELECT * FROM @relation WHERE NOT (@where_condition)",
        ),
        (
            "prefix_check",
            "SELECT * FROM @relation WHERE @'prefix' = ''",
        ),
    ]
    .into_iter()
    .map(|(name, body)| AuthoredFile {
        role: SqlFileRole::Audit,
        path: format!("sql/audits/generic/{name}.sql"),
        contents: format!("AUDIT ();\n{body}\n"),
        texts: vec![body.to_owned()],
    })
    .collect();
    let mut rename = request(RefactorOperation::RenameColumn, "is_marked", None);
    rename.column_name = Some("is_flagged".to_owned());
    let scan = context("duckdb");
    let rename_context = column_rename_context(&project_facts, &rename, &scan)?;
    let parts = header_parts(&rename_context, &orders)?;
    let edits: Vec<TextEdit> = parts.edits.into_iter().map(|(_, edit)| edit).collect();
    let manual = parts
        .manual
        .into_iter()
        .map(|item| {
            format!(
                "{}:{} {}",
                item.line.unwrap_or_default(),
                item.column.unwrap_or_default(),
                item.reason
            )
        })
        .collect();
    Ok((described(&edits), manual))
}

/// Planned test file changes with their replacements, and the plan's notes.
type FollowedTests = (Vec<(String, Vec<String>)>, Vec<String>);

/// Planned test file changes and `path: reason` notes when the scope host reports `rebound`.
pub(super) fn followed_unit_tests(
    request: &RefactorRequest,
    tests: &[(&str, &str, &[&str])],
    rebound: &[&str],
) -> Result<FollowedTests, RefactorError> {
    let mut orders: ModelFacts = model("orders", Some("view"), "analytics");
    orders.path = "sql/models/marts/orders.sql".to_owned();
    let mut project_facts = facts("/nonexistent/project", vec![orders]);
    for (path, name, targets) in tests {
        project_facts.authored_files.push(AuthoredFile {
            role: SqlFileRole::Test,
            path: (*path).to_owned(),
            contents: format!("TEST (name \"{name}\");\n\nSELECT 1\n"),
            texts: vec!["SELECT 1".to_owned()],
        });
        project_facts.unit_tests.push(UnitTestFacts {
            path: (*path).to_owned(),
            name: Some((*name).to_owned()),
            target_model_names: targets.iter().map(|target| (*target).to_owned()).collect(),
        });
    }
    let host = |_: &[(String, String)]| {
        Ok(DeclarationPlacement {
            relocated: Some(Vec::new()),
            ..DeclarationPlacement::default()
        })
    };
    let test_host = |_: &str, _: &str| {
        Ok(Some(
            rebound.iter().map(|item| (*item).to_owned()).collect(),
        ))
    };
    let plan = crate::refactoring::main::plan_refactor::plan_refactor(
        &project_facts,
        std::slice::from_ref(request),
        &host,
        &test_host,
    )?;
    let notes: Vec<String> = plan
        .notes
        .iter()
        .map(|note| format!("{}: {}", note.path, note.reason))
        .collect();
    let mut changes: Vec<(String, Vec<String>)> = plan
        .changes
        .into_iter()
        .filter(|change| change.original_path.starts_with("tests/"))
        .map(|change| {
            (
                format!("{} -> {}", change.original_path, change.path),
                change
                    .edits
                    .into_iter()
                    .map(|edit| edit.replacement)
                    .collect(),
            )
        })
        .collect();
    changes.sort();
    Ok((changes, notes))
}

/// Expected `original -> path` test file changes with their replacements, sorted by path.
pub(super) fn followed_test_changes(changes: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
    let mut expected = changes
        .iter()
        .map(|(path, edits)| {
            (
                (*path).to_owned(),
                edits.iter().map(|edit| (*edit).to_owned()).collect(),
            )
        })
        .collect::<Vec<_>>();
    expected.sort();
    expected
}

/// A move of `model` to `destination`.
pub(super) fn move_request(model: &str, destination: &str) -> RefactorRequest {
    RefactorRequest {
        operation: RefactorOperation::MoveModel,
        model_name: model.to_owned(),
        new_name: String::new(),
        column_name: None,
        destination: Some(destination.to_owned()),
        cascade: false,
    }
}

/// The `path: reason` blockers and `original -> path` file moves of planning `requests` together.
pub(super) fn planned_batch(
    facts: &RefactorFacts,
    requests: &[RefactorRequest],
) -> Result<(Vec<String>, Vec<String>), RefactorError> {
    let host = |_: &[(String, String)]| {
        Ok(DeclarationPlacement {
            relocated: Some(Vec::new()),
            ..DeclarationPlacement::default()
        })
    };
    let test_host = |_: &str, _: &str| Ok(Some(Vec::new()));
    let plan =
        crate::refactoring::main::plan_refactor::plan_refactor(facts, requests, &host, &test_host)?;
    Ok((
        plan.blocking
            .iter()
            .map(|item| format!("{}: {}", item.path, item.reason))
            .collect(),
        plan.changes
            .iter()
            .filter(|change| change.moved())
            .map(|change| format!("{} -> {}", change.original_path, change.path))
            .collect(),
    ))
}

/// A change that moves `original_path` to `path` without edits.
pub(super) fn moved(original_path: &str, path: &str) -> FileChange {
    FileChange {
        path: path.to_owned(),
        original_path: original_path.to_owned(),
        edits: Vec::new(),
    }
}

/// Each of `paths` that is a file below `root`, with its contents, sorted by path.
pub(super) fn tree(root: &Path, paths: &[&str]) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = paths
        .iter()
        .filter(|path| root.join(path).is_file())
        .map(|path| {
            (
                (*path).to_owned(),
                fs::read_to_string(root.join(path)).unwrap_or_default(),
            )
        })
        .collect();
    files.sort();
    files.dedup();
    files
}
