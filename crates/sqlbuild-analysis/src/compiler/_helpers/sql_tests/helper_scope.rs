//! Helper and mock CTEs placed in scope for SQL-test queries, in dependency order.

use std::collections::{BTreeMap, HashMap, HashSet};

use sqlbuild_core::graph_walk::main::depth_first_postorder::depth_first_postorder;

use rustc_hash::FxHashSet;

use crate::analysis_route::types::NativeDialect;
use crate::compiler::_helpers::sql_tests::cte_rename::defined_cte_keys;
use crate::compiler::_helpers::sql_tests::cte_slices::SliceDialect;
use crate::compiler::_helpers::sql_tests::cte_sql::{with_leading_ctes, with_unique_ctes};
use crate::compiler::_helpers::sql_tests::helper_names::{
    helper_cte_name, rename_helper_references,
};
use crate::compiler::_helpers::sql_tests::markers::{in_protected_range, protected_ranges};
use crate::compiler::_helpers::sql_tests::planning::{
    DBT_REF_PREFIX, REF_PREFIX, SEED_PREFIX, SOURCE_PREFIX, SqlTestPatterns, TABLE_FUNCTION_PREFIX,
    TestFixtures, compile_error,
};
use crate::compiler::_helpers::sql_tests::reader_references::calls_reference;
use crate::constants::{
    QUOTED_IDENTIFIER_DELIMITER_BYTES, SQL_TEST_ACTUAL_CTE, SQL_TEST_ACTUAL_CTE_PREFIX,
    SQL_TEST_EXPECTED_CTE,
};

/// Helper CTEs, and the mocks they read by CTE name, that one test query needs in scope.
pub(crate) struct HelperScope {
    pub(crate) ctes: Vec<(String, String)>,
    pub(crate) reached_mocks: HashSet<String>,
    /// Mock and model CTEs resolved helpers read, which defer to the reader's own copy.
    pub(crate) generated: HashSet<String>,
}

/// One mock CTE other test CTEs can read by its generated name.
#[derive(Debug)]
pub(crate) struct ScopeMock {
    pub(crate) generated_name: String,
    pub(crate) mock_name: String,
    pub(crate) sql: String,
    tokens: Vec<String>,
}

/// One test-defined helper: its authored name and body, and its top-level CTE name and body.
#[derive(Debug)]
struct ScopeHelper {
    name: String,
    original: String,
    cte_name: String,
    sql: String,
    tokens: Vec<String>,
    resolved: Option<ResolvedHelper>,
}

/// A helper whose relation references were resolved like an assertion's, after the model chain.
#[derive(Debug)]
pub(crate) struct ResolvedHelper {
    /// The helper body with every reference replaced by the CTE that stands in for it.
    pub(crate) sql: String,
    /// The mock and model CTEs the resolved body reads, dependencies first.
    pub(crate) lifted_ctes: Vec<(String, String)>,
    pub(crate) reached_mocks: HashSet<String>,
}

/// One test CTE another test CTE reads: a mock, or a helper when `mock_name` is `None`.
pub(crate) struct ScopeDependency<'a> {
    pub(crate) generated_name: &'a str,
    pub(crate) mock_name: Option<&'a str>,
    pub(crate) sql: &'a str,
}

/// Where helper CTEs live in the rendered test query.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum HelperPlacement {
    /// Each top-level test CTE carries the helpers it reads in its own WITH.
    #[default]
    Inline,
    /// Helpers are top-level CTEs under the names their readers were rewritten to.
    TopLevel,
}

/// Inputs that decide how one test's helpers are named and placed.
pub(crate) struct ScopeRequest<'a> {
    pub(crate) fixtures: &'a TestFixtures,
    pub(crate) patterns: &'a SqlTestPatterns,
    pub(crate) file_label: &'a str,
    pub(crate) rename_dialect: Option<&'a dyn NativeDialect>,
    pub(crate) flat_dialect: bool,
    pub(crate) slice_dialect: SliceDialect,
    /// Helpers the compiler found the test reads; an unread helper calling a reference is left out.
    pub(crate) read_helper_names: &'a [String],
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ScopeNode {
    Helper(usize),
    Mock(usize),
}

/// Name references between one test's helper and mock CTEs, tokenized once per test.
#[derive(Debug, Default)]
pub(crate) struct ScopeGraph {
    placement: HelperPlacement,
    helpers: Vec<ScopeHelper>,
    helper_indexes: HashMap<String, usize>,
    mocks: Vec<ScopeMock>,
    mock_indexes: HashMap<String, usize>,
    mocks_by_generated_name: HashMap<String, usize>,
    rewritten: HashMap<String, String>,
}

impl ScopeGraph {
    /// Tokenize one test's helpers and mocks and decide how its helpers are named and placed.
    pub(crate) fn new(request: &ScopeRequest<'_>) -> Result<Self, String> {
        let fixtures = request.fixtures;
        let patterns = request.patterns;
        let mock_groups: [(&str, &BTreeMap<String, String>); 4] = [
            (REF_PREFIX, &fixtures.mock_refs),
            (SOURCE_PREFIX, &fixtures.mock_sources),
            (SEED_PREFIX, &fixtures.mock_seeds),
            (DBT_REF_PREFIX, &fixtures.mock_dbt_refs),
        ];
        let mut graph = Self::default();
        for (prefix, group) in mock_groups {
            for (name, body) in group {
                graph.mocks.push(ScopeMock {
                    generated_name: format!("{prefix}{name}"),
                    mock_name: name.clone(),
                    sql: body.clone(),
                    tokens: identifier_tokens(body, patterns),
                });
            }
        }
        for (index, mock) in graph.mocks.iter().enumerate() {
            graph
                .mock_indexes
                .insert(mock.generated_name.to_ascii_lowercase(), index);
            graph
                .mocks_by_generated_name
                .insert(mock.generated_name.clone(), index);
        }
        let read: HashSet<String> = request
            .read_helper_names
            .iter()
            .map(|name| name.to_ascii_lowercase())
            .collect();
        for cte in &fixtures.helpers {
            let key = cte.name.to_ascii_lowercase();
            if !read.contains(&key) && calls_reference(&cte.sql_body, patterns) {
                continue;
            }
            graph.helper_indexes.insert(key, graph.helpers.len());
            graph.helpers.push(ScopeHelper {
                name: cte.name.clone(),
                original: cte.sql_body.clone(),
                cte_name: cte.name.clone(),
                sql: cte.sql_body.clone(),
                tokens: identifier_tokens(&cte.sql_body, patterns),
                resolved: None,
            });
        }
        if graph.helpers.is_empty() {
            return Ok(graph);
        }
        let readers = test_readers(fixtures);
        graph.reject_shadowed_helpers(&readers, request)?;
        if let Some(dialect) = request.rename_dialect
            && let Some(rewritten) = graph.rewrite_readers(&readers, dialect, patterns)
        {
            graph.placement = HelperPlacement::TopLevel;
            for helper in &mut graph.helpers {
                helper.cte_name = helper_cte_name(&helper.name);
                if let Some(sql) = rewritten.get(&helper.original) {
                    helper.sql = sql.clone();
                }
            }
            for mock in &mut graph.mocks {
                if let Some(sql) = rewritten.get(&mock.sql) {
                    mock.sql = sql.clone();
                }
            }
            graph.rewritten = rewritten;
            return Ok(graph);
        }
        if request.flat_dialect {
            graph.placement = HelperPlacement::TopLevel;
            return Ok(graph);
        }
        let inline: Vec<String> = graph
            .mocks
            .iter()
            .map(|mock| graph.with_read_helpers(&mock.sql, &mock.tokens))
            .collect();
        for (mock, sql) in graph.mocks.iter_mut().zip(inline) {
            mock.sql = sql;
        }
        Ok(graph)
    }

    /// Reject a test CTE that redefines a helper's name in its own WITH, which no dialect scopes alike.
    fn reject_shadowed_helpers(
        &self,
        readers: &[(String, &str)],
        request: &ScopeRequest<'_>,
    ) -> Result<(), String> {
        for (label, sql) in readers {
            for key in defined_cte_keys(sql, request.slice_dialect) {
                if let Some(index) = self.helper_indexes.get(&key) {
                    return Err(compile_error(&format!(
                        "SQL test '{}' defines CTE '{}' inside CTE '{label}', which redefines \
                         helper CTE '{}'; give one of them a different name",
                        request.file_label, key, self.helpers[*index].name
                    )));
                }
            }
        }
        Ok(())
    }

    /// Rewrite helper references in every test CTE, or `None` when any one cannot be rewritten.
    fn rewrite_readers(
        &self,
        readers: &[(String, &str)],
        dialect: &dyn NativeDialect,
        patterns: &SqlTestPatterns,
    ) -> Option<HashMap<String, String>> {
        let names: HashMap<String, String> = self
            .helpers
            .iter()
            .map(|helper| {
                (
                    helper.name.to_ascii_lowercase(),
                    helper_cte_name(&helper.name),
                )
            })
            .collect();
        let mut rewritten: HashMap<String, String> = HashMap::new();
        for (_, sql) in readers {
            if rewritten.contains_key(*sql)
                || !identifier_tokens(sql, patterns)
                    .iter()
                    .any(|token| self.helper_indexes.contains_key(token))
            {
                continue;
            }
            rewritten.insert(
                (*sql).to_string(),
                rename_helper_references(sql, &names, dialect)?,
            );
        }
        Some(rewritten)
    }

    fn node(&self, token: &str) -> Option<ScopeNode> {
        self.helper_indexes
            .get(token)
            .map(|index| ScopeNode::Helper(*index))
            .or_else(|| {
                self.mock_indexes
                    .get(token)
                    .map(|index| ScopeNode::Mock(*index))
            })
    }

    fn tokens(&self, node: ScopeNode) -> &[String] {
        match node {
            ScopeNode::Helper(index) => &self.helpers[index].tokens,
            ScopeNode::Mock(index) => &self.mocks[index].tokens,
        }
    }

    fn closure(&self, tokens: &[String], through_mocks: bool) -> Vec<ScopeNode> {
        let entered = |tokens: &[String]| -> Vec<ScopeNode> {
            tokens
                .iter()
                .filter_map(|token| self.node(token))
                .filter(|node| through_mocks || !matches!(node, ScopeNode::Mock(_)))
                .collect()
        };
        depth_first_postorder(entered(tokens), |node| entered(self.tokens(*node)))
    }

    /// Indexes of the helpers the compiler found the test's expected rows and assertions read.
    pub(crate) fn read_helper_indexes(&self, names: &[String]) -> HashSet<usize> {
        names
            .iter()
            .filter_map(|name| self.helper_indexes.get(&name.to_ascii_lowercase()).copied())
            .collect()
    }

    /// Read helpers whose SQL, as placed in the test query, calls a reference such as `__ref()`.
    pub(crate) fn referencing_helpers(
        &self,
        read: &HashSet<usize>,
        patterns: &SqlTestPatterns,
    ) -> Vec<(usize, String)> {
        self.helpers
            .iter()
            .enumerate()
            .filter(|(index, helper)| helper.resolved.is_none() && read.contains(index))
            .filter_map(|(index, _)| {
                let sql = self.placed_helper_sql(index);
                (patterns.test_reference.is_match(sql) || patterns.udf.is_match(sql))
                    .then(|| (index, sql.to_string()))
            })
            .collect()
    }

    /// The authored name of one helper.
    pub(crate) fn helper_name(&self, index: usize) -> &str {
        &self.helpers[index].name
    }

    /// Replace a helper's references with the CTEs a reader must define ahead of it.
    pub(crate) fn resolve_helper(&mut self, index: usize, resolved: ResolvedHelper) {
        let placement = self.placement;
        let Some(helper) = self.helpers.get_mut(index) else {
            return;
        };
        match placement {
            HelperPlacement::TopLevel => helper.sql.clone_from(&resolved.sql),
            HelperPlacement::Inline => helper.original.clone_from(&resolved.sql),
        }
        helper.resolved = Some(resolved);
    }

    fn placed_helper_sql(&self, index: usize) -> &str {
        match self.placement {
            HelperPlacement::TopLevel => &self.helpers[index].sql,
            HelperPlacement::Inline => &self.helpers[index].original,
        }
    }

    /// Whether a CTE name is a test helper placed at the top level of the test query.
    pub(crate) fn is_top_level_helper(&self, name: &str) -> bool {
        self.placement == HelperPlacement::TopLevel
            && self.helpers.iter().any(|helper| helper.cte_name == name)
    }

    /// The body of one mock as its own top-level CTE.
    pub(crate) fn mock_sql(&self, generated_name: &str) -> Option<&str> {
        self.mocks_by_generated_name
            .get(generated_name)
            .map(|index| self.mocks[*index].sql.as_str())
    }

    /// One test-authored query as placed in the test query: rewritten, or carrying its helpers.
    pub(crate) fn reader_sql(&self, sql: &str, patterns: &SqlTestPatterns) -> String {
        match self.placement {
            HelperPlacement::TopLevel => self
                .rewritten
                .get(sql)
                .cloned()
                .unwrap_or_else(|| sql.to_string()),
            HelperPlacement::Inline => {
                self.with_read_helpers(sql, &identifier_tokens(sql, patterns))
            }
        }
    }

    fn with_read_helpers(&self, sql: &str, tokens: &[String]) -> String {
        let helpers: Vec<(String, String)> = self
            .closure(tokens, false)
            .into_iter()
            .filter_map(|node| match node {
                ScopeNode::Helper(index) => Some((
                    self.helpers[index].name.clone(),
                    self.helpers[index].original.clone(),
                )),
                ScopeNode::Mock(_) => None,
            })
            .collect();
        with_leading_ctes(&helpers, sql)
    }

    /// Top-level CTEs one top-level mock reads, transitively and dependencies first.
    pub(crate) fn mock_dependencies(&self, generated_name: &str) -> Vec<ScopeDependency<'_>> {
        let Some(root) = self.mocks_by_generated_name.get(generated_name).copied() else {
            return Vec::new();
        };
        self.closure(&self.mocks[root].tokens, true)
            .into_iter()
            .filter_map(|node| self.dependency(node, Some(root)))
            .collect()
    }

    /// Top-level CTEs a test-authored query placed inline in model SQL reads.
    pub(crate) fn reader_scope(&self, sql: &str, patterns: &SqlTestPatterns) -> HelperScope {
        self.collect_scope(self.closure(&identifier_tokens(sql, patterns), true))
    }

    /// Top-level CTEs `sql` reads, transitively and dependencies first; mocks only when asked.
    pub(crate) fn reader_ctes(
        &self,
        sql: &str,
        patterns: &SqlTestPatterns,
        through_mocks: bool,
    ) -> Vec<(String, String)> {
        self.collect_scope(self.closure(&identifier_tokens(sql, patterns), through_mocks))
            .ctes
    }

    fn collect_scope(&self, nodes: Vec<ScopeNode>) -> HelperScope {
        let mut scope = HelperScope {
            ctes: Vec::new(),
            reached_mocks: HashSet::new(),
            generated: HashSet::new(),
        };
        for node in nodes {
            if let ScopeNode::Mock(index) = node {
                scope
                    .reached_mocks
                    .insert(self.mocks[index].mock_name.clone());
            }
            if let ScopeNode::Helper(index) = node
                && let Some(resolved) = &self.helpers[index].resolved
            {
                scope
                    .reached_mocks
                    .extend(resolved.reached_mocks.iter().cloned());
                scope.generated.extend(
                    resolved
                        .lifted_ctes
                        .iter()
                        .map(|(name, _)| name.to_ascii_lowercase()),
                );
                scope.ctes = with_unique_ctes(
                    std::mem::take(&mut scope.ctes),
                    resolved.lifted_ctes.iter().cloned(),
                );
            }
            if let Some(dependency) = self.dependency(node, None) {
                scope.ctes = with_unique_ctes(
                    std::mem::take(&mut scope.ctes),
                    [(
                        dependency.generated_name.to_string(),
                        dependency.sql.to_string(),
                    )],
                );
            }
        }
        scope
    }

    fn dependency(&self, node: ScopeNode, root: Option<usize>) -> Option<ScopeDependency<'_>> {
        match node {
            ScopeNode::Mock(index) if Some(index) == root => None,
            ScopeNode::Mock(index) => Some(ScopeDependency {
                generated_name: &self.mocks[index].generated_name,
                mock_name: Some(&self.mocks[index].mock_name),
                sql: &self.mocks[index].sql,
            }),
            ScopeNode::Helper(index) => {
                (self.placement == HelperPlacement::TopLevel).then(|| ScopeDependency {
                    generated_name: &self.helpers[index].cte_name,
                    mock_name: None,
                    sql: &self.helpers[index].sql,
                })
            }
        }
    }
}

/// Every test-authored query that may read a helper, labelled by its CTE name.
fn test_readers(fixtures: &TestFixtures) -> Vec<(String, &str)> {
    let mut readers: Vec<(String, &str)> = Vec::new();
    readers.extend(
        fixtures
            .helpers
            .iter()
            .map(|cte| (cte.name.clone(), cte.sql_body.as_str())),
    );
    for (prefix, group) in [
        (REF_PREFIX, &fixtures.mock_refs),
        (SOURCE_PREFIX, &fixtures.mock_sources),
        (SEED_PREFIX, &fixtures.mock_seeds),
        (DBT_REF_PREFIX, &fixtures.mock_dbt_refs),
        (TABLE_FUNCTION_PREFIX, &fixtures.mock_table_functions),
    ] {
        readers.extend(
            group
                .iter()
                .map(|(name, sql)| (format!("{prefix}{name}"), sql.as_str())),
        );
    }
    readers.extend(
        fixtures
            .expected
            .iter()
            .map(|(name, sql)| (format!("__expected__{name}"), sql.as_str())),
    );
    readers.extend(
        fixtures
            .assertions
            .iter()
            .map(|(name, sql)| (format!("__assert__{name}"), sql.as_str())),
    );
    readers
}

/// Return the helper and mock CTEs `sql` reads, transitively and dependencies first.
pub(crate) fn helper_scope_ctes(
    sql: &str,
    fixtures: &TestFixtures,
    patterns: &SqlTestPatterns,
    file_label: &str,
) -> Result<HelperScope, String> {
    let graph = &fixtures.scope;
    if graph.helpers.is_empty() && graph.mocks.is_empty() {
        return Ok(HelperScope {
            ctes: Vec::new(),
            reached_mocks: HashSet::new(),
            generated: HashSet::new(),
        });
    }
    let nodes = graph.closure(&identifier_tokens(sql, patterns), true);
    for node in &nodes {
        if let ScopeNode::Helper(index) = node
            && is_generated_cte_name(&graph.helpers[*index].name)
        {
            return Err(compile_error(&format!(
                "SQL test '{file_label}' defines CTE '{}', which conflicts with the generated CTE",
                graph.helpers[*index].name
            )));
        }
    }
    Ok(graph.collect_scope(nodes))
}

/// Append scoped CTEs after a step's CTEs; a same-named mock or model CTE keeps the step's copy.
pub(crate) fn merged_scoped_ctes(
    lifted: Vec<(String, String)>,
    scoped: HelperScope,
    file_label: &str,
) -> Result<Vec<(String, String)>, String> {
    let mut merged: Vec<(String, String)> = lifted;
    for (name, sql) in scoped.ctes {
        match merged
            .iter()
            .find(|(existing, _)| existing.eq_ignore_ascii_case(&name))
        {
            Some((_, existing_sql))
                if *existing_sql == sql
                    || scoped.generated.contains(&name.to_ascii_lowercase()) => {}
            Some(_) => {
                return Err(compile_error(&format!(
                    "SQL test '{file_label}' defines CTE '{name}', which conflicts with the generated CTE"
                )));
            }
            None => merged.push((name, sql)),
        }
    }
    Ok(merged)
}

/// Names the comparison renderer generates, which a scoped helper CTE must not shadow.
fn is_generated_cte_name(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    lowered == SQL_TEST_ACTUAL_CTE
        || lowered == SQL_TEST_EXPECTED_CTE
        || lowered.starts_with(SQL_TEST_ACTUAL_CTE_PREFIX)
}

/// Distinct lowercase identifiers outside strings and comments, in first-appearance order.
fn identifier_tokens(sql: &str, patterns: &SqlTestPatterns) -> Vec<String> {
    let protected = protected_ranges(&patterns.lexical, sql);
    let mut seen: FxHashSet<String> = FxHashSet::default();
    let mut tokens: Vec<String> = Vec::new();
    let mut push = |token: &str| {
        let lowered = token.to_ascii_lowercase();
        if seen.insert(lowered.clone()) {
            tokens.push(lowered);
        }
    };
    for found in patterns.identifier.find_iter(sql) {
        let preceded_by_identifier = sql[..found.start()]
            .chars()
            .next_back()
            .is_some_and(|character| character.is_alphanumeric() || character == '_');
        if !preceded_by_identifier && !in_protected_range(found.start(), &protected) {
            push(found.as_str());
        }
    }
    for (start, end) in protected {
        let text = &sql[start..end];
        if let Some(quote) = text
            .chars()
            .next()
            .filter(|quote| matches!(quote, '"' | '`'))
            && text.len() >= QUOTED_IDENTIFIER_DELIMITER_BYTES
            && text.ends_with(quote)
        {
            push(&text[1..text.len() - 1]);
        }
    }
    tokens
}
