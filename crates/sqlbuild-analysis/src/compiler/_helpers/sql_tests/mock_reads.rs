//! Python's `report_mocks_reading_referencing_helpers`: P013s for mocks reading referencing helpers.

use std::collections::HashSet;
use std::convert::Infallible;

use sqlbuild_core::graph_walk::main::breadth_first_reach::breadth_first_reach;
use sqlbuild_core::graph_walk::models::VisitOrder;
use sqlbuild_core::graph_walk::types::NodeSet;
use sqlbuild_core::text::main::ignorecase_key::ignorecase_key;
use sqlbuild_core::text::main::is_word_char::is_word_char;
use sqlbuild_sqltext::sql_references::main::extract_sql_references::extract_sql_references;
use sqlbuild_sqltext::sql_references::models::{ReferenceExtraction, SqlReference};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;
use sqlquack_syntax::expressions::queries::models::With;
use sqlquack_syntax::{Expression, ExpressionWalk, Parsed};

use crate::analysis_route::types::NativeDialect;
use crate::compiler::_helpers::sql_tests::planning::MOCK_CTE_PREFIXES;
use crate::compiler::models::{
    SqlTestAssemblyFailure, SqlTestAssemblyModel, SqlTestAssemblyModelPayload,
    SqlTestAssemblyReference, SqlTestAssemblyTest, SqlTestCte, SqlTestHelperDiagnostic,
};

const REF_KIND: &str = "ref";
const TABLE_FUNCTION_KIND: &str = "table_fn";
const DBT_REF_KIND: &str = "dbt_ref";
const RELATION_KINDS: [&str; 5] = [
    REF_KIND,
    "source",
    "seed",
    DBT_REF_KIND,
    TABLE_FUNCTION_KIND,
];

type Deferrable<T> = Result<T, SqlTestAssemblyFailure>;

/// A reference as compile carries it: the scanner's or a model input's.
#[derive(Clone)]
struct Reference {
    kind: String,
    name: String,
    package: Option<String>,
}

/// One model test whose mocks may read referencing helpers, with the project's models.
pub(crate) struct MockReadsRequest<'a> {
    pub(crate) test: &'a SqlTestAssemblyTest,
    pub(crate) payload: &'a SqlTestAssemblyModelPayload,
    pub(crate) models: &'a [SqlTestAssemblyModel],
    pub(crate) target_model_names: &'a [String],
    pub(crate) syntax: &'a LexicalSyntax,
    /// The project's native parser; without one, reads come from the token scan.
    pub(crate) parser: Option<&'a dyn NativeDialect>,
}

/// A Python dict by string key: a repeated key keeps its first position and takes the new value.
struct OrderedEntries<T> {
    entries: Vec<(String, T)>,
}

impl<T> OrderedEntries<T> {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn assign(&mut self, key: String, value: T) {
        match self
            .entries
            .iter_mut()
            .find(|(candidate, _)| *candidate == key)
        {
            Some(entry) => entry.1 = value,
            None => self.entries.push((key, value)),
        }
    }

    fn get(&self, key: &str) -> Option<&T> {
        self.entries
            .iter()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, value)| value)
    }

    fn keys(&self) -> Vec<String> {
        self.entries.iter().map(|(key, _)| key.clone()).collect()
    }
}

/// Python's `SqlTestCteGraph`, keyed by case-folded CTE name in first-definition order.
struct CteGraph<'a> {
    ctes: OrderedEntries<&'a SqlTestCte>,
    reads: OrderedEntries<Vec<String>>,
    reader_reads: Vec<String>,
}

/// A located span as Python's `reference_call_location` reports it.
struct Span {
    line: usize,
    column: usize,
    end_line: usize,
    end_column: usize,
}

impl CteGraph<'_> {
    fn cte(&self, key: &str) -> Option<&SqlTestCte> {
        self.ctes.get(key).copied()
    }

    fn reads(&self, key: &str) -> &[String] {
        self.reads.get(key).map_or(&[], Vec::as_slice)
    }

    /// Python's `reachable_cte_keys`: breadth first, roots included, in visit order.
    fn reachable(&self, roots: &[String]) -> Vec<String> {
        let known: Vec<&String> = roots
            .iter()
            .filter(|root| self.cte(root).is_some())
            .collect();
        let mut visited: VisitOrder<&String> = VisitOrder::default();
        for root in &known {
            let Ok(_) = visited.insert_new(root);
        }
        let Ok(()) = breadth_first_reach(known, None, &mut visited, |key| {
            Ok::<_, Infallible>(self.reads(key))
        });
        visited.order.into_iter().cloned().collect()
    }
}

/// The P013 diagnostics Python reports for one model test, in report order.
pub(crate) fn mock_reading_helper_diagnostics(
    request: &MockReadsRequest<'_>,
) -> Deferrable<Vec<SqlTestHelperDiagnostic>> {
    let payload = request.payload;
    let mut referencing: OrderedEntries<Reference> = OrderedEntries::new();
    for cte in &payload.authored_ctes {
        if is_mock_name(&cte.name) {
            continue;
        }
        let first = references(&cte.sql_body, request.syntax)?
            .into_iter()
            .find(|reference| RELATION_KINDS.contains(&reference.kind.as_str()));
        if let Some(reference) = first {
            referencing.assign(fold(&cte.name), reference);
        }
    }
    if referencing.entries.is_empty() {
        return Ok(Vec::new());
    }
    let readers: Vec<&SqlTestCte> = payload
        .expected_ctes
        .iter()
        .chain(&payload.assertion_ctes)
        .collect();
    let graph = cte_graph(request.parser, &payload.authored_ctes, &readers)?;
    let mut scanned: Vec<&str> = readers.iter().map(|cte| cte.sql_body.as_str()).collect();
    for key in read_helper_keys(&graph) {
        if let Some(cte) = graph.cte(&key) {
            scanned.push(cte.sql_body.as_str());
        }
    }
    let mut called_mocks: Vec<String> = Vec::new();
    for sql in scanned {
        for reference in references(sql, request.syntax)? {
            called_mocks.extend(mock_keys(&reference)?);
        }
    }
    let used = used_mock_keys(request, &graph, &called_mocks)?;
    let mut diagnostics: Vec<SqlTestHelperDiagnostic> = Vec::new();
    for mock_key in used {
        let mut helper: Option<(String, &Reference)> = None;
        for key in graph.reachable(graph.reads(&mock_key)) {
            if let Some(reference) = referencing.get(&key) {
                helper = Some((key, reference));
                break;
            }
        }
        let Some((helper_key, reference)) = helper else {
            continue;
        };
        let (Some(mock), Some(helper)) = (graph.cte(&mock_key), graph.cte(&helper_key)) else {
            continue;
        };
        let call = reference_call(reference);
        let span = located(request.test, &helper.name, &call);
        diagnostics.push(SqlTestHelperDiagnostic {
            line: span.line,
            column: span.column,
            end_line: span.end_line,
            end_column: span.end_column,
            message: format!(
                "SQL test mock '{}' reads helper CTE '{}', which calls {call}; mocks and \
                 fixtures are defined before the models the test runs, so the helper cannot be \
                 resolved for them",
                mock.name, helper.name
            ),
            help: mock_reference_help(&mock.name, &call, reference),
        });
    }
    Ok(diagnostics)
}

fn is_mock_name(name: &str) -> bool {
    MOCK_CTE_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// Python's `str.casefold`.
fn fold(text: &str) -> String {
    str::to_lowercase(text)
}

/// Python's `scan_sql_reference_calls(...).references`; a failing scan raises unlocated.
fn references(sql: &str, syntax: &LexicalSyntax) -> Deferrable<Vec<Reference>> {
    match extract_sql_references(sql, syntax) {
        ReferenceExtraction::Extracted(scan) => {
            Ok(scan.references.into_iter().map(scanned).collect())
        }
        ReferenceExtraction::Failed(failure) => Err(SqlTestAssemblyFailure::Input(failure.message)),
    }
}

fn scanned(reference: SqlReference) -> Reference {
    Reference {
        kind: reference.kind.to_owned(),
        name: reference.name,
        package: reference.package,
    }
}

fn model_reference(reference: &SqlTestAssemblyReference) -> Reference {
    Reference {
        kind: reference.kind.clone(),
        name: reference.name.clone(),
        package: reference.package.clone(),
    }
}

/// Python's `sql_test_cte_graph`.
fn cte_graph<'a>(
    parser: Option<&dyn NativeDialect>,
    authored: &'a [SqlTestCte],
    readers: &[&SqlTestCte],
) -> Deferrable<CteGraph<'a>> {
    let mut ctes: OrderedEntries<&'a SqlTestCte> = OrderedEntries::new();
    for cte in authored {
        ctes.assign(fold(&cte.name), cte);
    }
    let keys: Vec<String> = ctes.keys();
    let mut reads: OrderedEntries<Vec<String>> = OrderedEntries::new();
    for (key, cte) in &ctes.entries {
        let others: Vec<String> = keys.iter().filter(|other| *other != key).cloned().collect();
        let cte_reads = match parsed_reads(parser, &cte.sql_body, &others) {
            Some(parsed) => parsed,
            None => token_reads(&cte.sql_body, &others),
        };
        reads.assign(key.clone(), cte_reads);
    }
    let mut reader_reads: Vec<String> = Vec::new();
    for reader in readers {
        let parsed = match parsed_reads(parser, &reader.sql_body, &keys) {
            Some(parsed) => parsed,
            None => token_reads(&reader.sql_body, &keys),
        };
        for key in parsed {
            if !reader_reads.contains(&key) {
                reader_reads.push(key);
            }
        }
    }
    Ok(CteGraph {
        ctes,
        reads,
        reader_reads,
    })
}

/// Python's `_parsed_reads`: unqualified reads of `keys` no nested CTE shadows, or None unparsed.
fn parsed_reads(
    parser: Option<&dyn NativeDialect>,
    sql: &str,
    keys: &[String],
) -> Option<Vec<String>> {
    let folded = fold(sql);
    if !keys.iter().any(|key| folded.contains(key.as_str())) {
        return Some(Vec::new());
    }
    let Ok(Parsed {
        mut statements,
        gaps,
    }) = parser?.parse(sql)
    else {
        return None;
    };
    if statements.len() != 1 || !gaps.is_empty() {
        return None;
    }
    let parsed: Expression = statements.remove(0);
    let nested = defined_cte_keys(&parsed);
    let mut reads: Vec<String> = Vec::new();
    for node in parsed.dfs() {
        let Expression::Table(table) = node else {
            continue;
        };
        if table.schema.is_some() || table.catalog.is_some() {
            continue;
        }
        let key = fold(&table.name.name);
        if keys.contains(&key) && !nested.contains(&key) && !reads.contains(&key) {
            reads.push(key);
        }
    }
    Some(reads)
}

/// Python's `_defined_cte_keys` over `to_dict()`.
fn defined_cte_keys(parsed: &Expression) -> HashSet<String> {
    let mut keys: HashSet<String> = HashSet::new();
    for node in parsed.dfs() {
        let with: Option<&With> = match node {
            Expression::Select(select) => select.with.as_ref(),
            Expression::Union(union) => union.with.as_ref(),
            Expression::Intersect(intersect) => intersect.with.as_ref(),
            Expression::Except(except) => except.with.as_ref(),
            Expression::Pivot(pivot) => pivot.with.as_ref(),
            Expression::Insert(insert) => insert.with.as_ref(),
            Expression::Update(update) => update.with.as_ref(),
            Expression::Delete(delete) => delete.with.as_ref(),
            Expression::CreateTable(create) => create.with_cte.as_ref(),
            Expression::With(with) => Some(with),
            _ => None,
        };
        for cte in with.map_or(&[][..], |with| with.ctes.as_slice()) {
            let _ = keys.insert(fold(&cte.alias.name));
        }
    }
    keys
}

/// Python's `_token_reads` over `[A-Za-z_][\w$]*` tokens.
fn token_reads(sql: &str, keys: &[String]) -> Vec<String> {
    let chars: Vec<(usize, char)> = sql.char_indices().collect();
    let mut tokens: Vec<String> = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if !(chars[index].1.is_ascii_alphabetic() || chars[index].1 == '_') {
            index += 1;
            continue;
        }
        let start = chars[index].0;
        index += 1;
        while index < chars.len() && is_word_or_dollar(chars[index].1) {
            index += 1;
        }
        let end = chars.get(index).map_or(sql.len(), |(offset, _)| *offset);
        let token = fold(&sql[start..end]);
        if !tokens.contains(&token) {
            tokens.push(token);
        }
    }
    tokens.retain(|token| keys.contains(token));
    tokens
}

/// Python's `_read_helper_keys`: non-mock CTEs the test's readers reach.
fn read_helper_keys(graph: &CteGraph<'_>) -> Vec<String> {
    graph
        .reachable(&graph.reader_reads)
        .into_iter()
        .filter(|key| graph.cte(key).is_some_and(|cte| !is_mock_name(&cte.name)))
        .collect()
}

/// Python's `_mock_keys`: the case-folded mock CTE names that stand in for `reference`.
fn mock_keys(reference: &Reference) -> Deferrable<Vec<String>> {
    let prefix = match reference.kind.as_str() {
        "ref" => "__ref__",
        "source" => "__source__",
        "seed" => "__seed__",
        "dbt_ref" => "__dbt_ref__",
        "table_fn" => "__table_fn__",
        _ => return Ok(Vec::new()),
    };
    let mut names: Vec<String> = vec![reference.name.clone()];
    if let Some(package) = &reference.package {
        names.push(format!("{package}__{}", reference.name));
    }
    names
        .iter()
        .map(|name| Ok(fold(&format!("{prefix}{name}"))))
        .collect()
}

/// Python's `_used_mock_keys`.
fn used_mock_keys(
    request: &MockReadsRequest<'_>,
    graph: &CteGraph<'_>,
    called_mocks: &[String],
) -> Deferrable<Vec<String>> {
    let mock_model_names = &request.payload.mock_model_names;
    let mut used: Vec<String> = graph
        .reachable(&graph.reader_reads)
        .into_iter()
        .filter(|key| graph.cte(key).is_some_and(|cte| is_mock_name(&cte.name)))
        .collect();
    let mut seen_called: Vec<&String> = Vec::new();
    for key in called_mocks {
        if seen_called.contains(&key) {
            continue;
        }
        seen_called.push(key);
        if graph.cte(key).is_some() && !used.contains(key) {
            used.push(key.clone());
        }
    }
    let mut model_references: OrderedEntries<&[SqlTestAssemblyReference]> = OrderedEntries::new();
    for model in request.models {
        model_references.assign(model.name.clone(), model.references.as_slice());
    }
    let mut pending: Vec<String> = request
        .target_model_names
        .iter()
        .filter(|name| !mock_model_names.contains(name))
        .cloned()
        .collect();
    let mut visited: HashSet<String> = pending.iter().cloned().collect();
    let mut cursor = 0;
    while cursor < pending.len() {
        let model_name = pending[cursor].clone();
        cursor += 1;
        let references: &[SqlTestAssemblyReference] = model_references
            .get(&model_name)
            .copied()
            .unwrap_or_default();
        for reference in references {
            let reference = model_reference(reference);
            for mock_key in mock_keys(&reference)? {
                if graph.cte(&mock_key).is_some() && !used.contains(&mock_key) {
                    used.push(mock_key);
                }
            }
            if reference.kind == REF_KIND
                && !mock_model_names.contains(&reference.name)
                && visited.insert(reference.name.clone())
            {
                pending.push(reference.name);
            }
        }
    }
    Ok(used)
}

/// Python's `SqlReferenceKind.example_call(*args, quote='"')`.
fn example_call(kind: &str, arguments: &[&str]) -> String {
    let quoted: Vec<String> = arguments
        .iter()
        .map(|argument| format!("\"{}\"", argument.replace('"', "\"\"")))
        .collect();
    format!("__{kind}({})", quoted.join(", "))
}

/// Python's `_reference_call`.
fn reference_call(reference: &Reference) -> String {
    match (&reference.package, reference.kind.as_str()) {
        (Some(package), DBT_REF_KIND) => example_call(&reference.kind, &[package, &reference.name]),
        _ => example_call(&reference.kind, &[&reference.name]),
    }
}

/// Python's `_mock_reference_help`.
fn mock_reference_help(mock_name: &str, call: &str, reference: &Reference) -> String {
    if reference.kind != TABLE_FUNCTION_KIND && reference.package.is_none() {
        let mock_cte = format!("__{}__{}", reference.kind, reference.name);
        return format!(
            "Read a mock by its CTE name instead, for example FROM {mock_cte} rather than \
             FROM {call}, defining {mock_cte} AS (SELECT ...) if the test does not mock it, \
             or write the rows of '{mock_name}' directly."
        );
    }
    format!("Write the rows of '{mock_name}' directly, for example {mock_name} AS (SELECT ...).")
}

/// Python's `sql_test_cte_location` and `reference_call_location`, in code points.
fn located(test: &SqlTestAssemblyTest, cte_name: &str, call: &str) -> Span {
    let contents: Vec<char> = test.contents.chars().collect();
    let block_offset: usize = test
        .contents
        .find(test.block_sql.as_str())
        .map_or(0, |byte| test.contents[..byte].chars().count());
    let name: Vec<char> = cte_name.chars().collect();
    let call: Vec<char> = call.chars().collect();
    let (start, length) = match cte_header(&contents, &name, block_offset) {
        Some((header_start, header_end)) => find_ignoring_case(&contents, &call, header_end)
            .map_or((header_start, name.len()), |found| (found, call.len())),
        None => (block_offset, 0),
    };
    let (line, column) = line_and_column(&contents, start);
    let (end_line, end_column) = line_and_column(&contents, start + length);
    Span {
        line,
        column,
        end_line,
        end_column,
    }
}

/// Python's `(?<![\w$])NAME["`]?\s+AS\s*\(` search from `from`, under `re.IGNORECASE`.
fn cte_header(contents: &[char], name: &[char], from: usize) -> Option<(usize, usize)> {
    let space = |index: usize| contents.get(index).is_some_and(|c| char::is_whitespace(*c));
    let mut start = from;
    while let Some(found) = find_ignoring_case(contents, name, start) {
        start = found + 1;
        if found > 0 && is_word_or_dollar(contents[found - 1]) {
            continue;
        }
        let mut cursor = found + name.len();
        if matches!(contents.get(cursor), Some('"' | '`')) {
            cursor += 1;
        }
        let spaces = cursor;
        while space(cursor) {
            cursor += 1;
        }
        if cursor == spaces || !matches_ignoring_case(contents, &['a', 's'], cursor) {
            continue;
        }
        cursor += 2;
        while space(cursor) {
            cursor += 1;
        }
        if contents.get(cursor) == Some(&'(') {
            return Some((found, cursor + 1));
        }
    }
    None
}

/// Python's `re.compile(re.escape(needle), re.IGNORECASE).search(contents, from)`.
fn find_ignoring_case(contents: &[char], needle: &[char], from: usize) -> Option<usize> {
    let needle: Vec<char> = needle
        .iter()
        .map(|character| ignorecase_key(*character))
        .collect();
    (from..=contents.len().checked_sub(needle.len())?)
        .find(|&index| keys_at(contents, &needle, index))
}

/// Whether `needle` matches at `at` under `re.IGNORECASE`.
fn matches_ignoring_case(contents: &[char], needle: &[char], at: usize) -> bool {
    let keys: Vec<char> = needle
        .iter()
        .map(|character| ignorecase_key(*character))
        .collect();
    keys_at(contents, &keys, at)
}

fn keys_at(contents: &[char], keys: &[char], at: usize) -> bool {
    contents.get(at..at + keys.len()).is_some_and(|window| {
        window
            .iter()
            .zip(keys)
            .all(|(character, key)| ignorecase_key(*character) == *key)
    })
}

/// Python's `[\w$]`.
fn is_word_or_dollar(character: char) -> bool {
    character == '$' || is_word_char(character)
}

fn line_and_column(contents: &[char], offset: usize) -> (usize, usize) {
    let before = &contents[..offset];
    let line = before
        .iter()
        .filter(|character| **character == '\n')
        .count()
        + 1;
    let line_start = before
        .iter()
        .rposition(|character| *character == '\n')
        .map_or(0, |index| index + 1);
    (line, offset - line_start + 1)
}
