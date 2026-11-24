use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use polyglot_sql::{DialectType, ExpressionWalk, SchemaValidationOptions};

use crate::semantic_checks::_helpers::explanation::messages::{missing_column, sentence_message};
use crate::semantic_checks::_helpers::metadata_checks::argument_types::expression_type;
use crate::semantic_checks::_helpers::sql_text::parsed_sql::{parsed_model, parsed_model_facts};
use crate::semantic_checks::main::check_semantic_metadata::check_semantic_metadata;
use crate::semantic_checks::main::complete_semantic_diagnostics::complete_semantic_diagnostics;
use crate::semantic_checks::main::finish_type_recovery::finish_type_recovery;
use crate::semantic_checks::main::plan_type_recovery::plan_type_recovery;
use crate::semantic_checks::models::{
    AuditSqlInput, CompletedDiagnostic, CompletionModel, CompletionRequest, DiagnosticOwner,
    FinalDiagnostic, HookModelInput, LineageOutput, MetadataFinding, MetadataFunction,
    MetadataModel, MetadataOutcome, MetadataRequest, MetadataSource, MetadataSqlTest, ModelBinding,
    ModelMetadataFindings, RawBinding, RecoveryModel, ResourceSqlRequest, SemanticDiagnostic,
    SemanticFailure, SemanticLocation, TypeRecoveryPlan, TypeRecoveryRequest, TypeRecoveryStep,
};
use crate::semantic_checks::tests::test_types::{
    CompletionTestCase, DescribedDiagnostic, MetadataTestCase, OperandTypeTestCase,
    TypeRecoveryTestCase,
};
use crate::semantic_validation::models::ProjectCatalog;

/// What type recovery decided: status, poisoned outputs, revalidated models, kept and bindings.
pub(crate) type RecoverySummary = (
    String,
    Vec<(String, String)>,
    Vec<usize>,
    Vec<(usize, Option<String>)>,
    Vec<Vec<usize>>,
);

/// What completion decided: its failure, final diagnostics and model binding positions.
pub(crate) type CompletionSummary = (
    Option<SemanticFailure>,
    Vec<DescribedDiagnostic>,
    Option<Vec<Vec<usize>>>,
);

pub(crate) fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

pub(crate) fn shape(columns: &[(&str, &str)]) -> Vec<(String, String)> {
    columns
        .iter()
        .map(|(name, kind)| ((*name).to_owned(), (*kind).to_owned()))
        .collect()
}

pub(crate) fn pairs(values: &[(&str, &str)]) -> BTreeSet<(String, String)> {
    shape(values).into_iter().collect()
}

pub(crate) fn names(values: &[&str]) -> BTreeSet<String> {
    strings(values).into_iter().collect()
}

pub(crate) fn sentence(message: &str) -> Result<String, SemanticFailure> {
    sentence_message(message)
}

/// The missing column and table Python reads from a message.
pub(crate) fn missing_parts(
    message: &str,
) -> Result<Option<(String, Option<String>)>, SemanticFailure> {
    missing_column(message)
}

/// An expected missing column and table as owned strings.
pub(crate) fn owned_missing(
    expected: Result<Option<(&str, Option<&str>)>, SemanticFailure>,
) -> Result<Option<(String, Option<String>)>, SemanticFailure> {
    expected.map(|missing| missing.map(owned_parts))
}

fn owned_parts((column, table): (&str, Option<&str>)) -> (String, Option<String>) {
    (column.to_owned(), table.map(str::to_owned))
}

/// Aliases and unaliased outputs of one DuckDB model.
pub(crate) fn parsed_facts(sql: &str) -> (BTreeSet<(String, String)>, BTreeSet<String>) {
    let facts = parsed_model_facts(sql, Some("duckdb")).expect("duckdb parses");
    (
        facts.aliases.into_iter().collect(),
        facts.unaliased_outputs.into_iter().collect(),
    )
}

pub(crate) fn catalog() -> ProjectCatalog {
    ProjectCatalog::with_options(
        DialectType::DuckDB,
        SchemaValidationOptions::default(),
        false,
    )
}

pub(crate) fn lineage(output: &str, upstream: &[(&str, &str)]) -> LineageOutput {
    LineageOutput {
        output_column: output.to_owned(),
        upstream: shape(upstream),
    }
}

fn recovery_model(
    name: &str,
    query_sql: &str,
    references: &[&str],
    lineage: Vec<LineageOutput>,
    binding: Option<(usize, &str, i64, i64)>,
    is_error: bool,
) -> RecoveryModel {
    let message = |code: &str| format!("{code} failed");
    RecoveryModel {
        name: name.to_owned(),
        query_sql: query_sql.to_owned(),
        inferred_columns: Some(
            lineage
                .iter()
                .map(|output| output.output_column.clone())
                .collect(),
        ),
        references: strings(references),
        lineage,
        bindings: binding
            .iter()
            .map(|(id, code, _, _)| ModelBinding {
                id: *id,
                code: (*code).to_owned(),
                message: message(code),
                is_error,
            })
            .collect(),
        raw_bindings: binding
            .iter()
            .map(|(id, code, start, end)| RawBinding {
                id: *id,
                code: (*code).to_owned(),
                message: message(code),
                start: Some(*start),
                end: Some(*end),
            })
            .collect(),
    }
}

fn owner(id: usize, code: &str, model: &str) -> DiagnosticOwner {
    DiagnosticOwner {
        id,
        code: code.to_owned(),
        is_model: true,
        resource_name: Some(model.to_owned()),
    }
}

/// A staging projection type error poisoning a mart output and a report reading it.
fn poisoned_chain(dialect: Option<&str>, is_error: bool) -> TypeRecoveryRequest {
    TypeRecoveryRequest {
        dialect: dialect.map(str::to_owned),
        models: vec![
            recovery_model(
                "stg",
                "SELECT order_id, amount + 'x' AS amount FROM __source(\"raw_orders\")",
                &["raw_orders"],
                vec![
                    lineage("order_id", &[("raw_orders", "order_id")]),
                    lineage("amount", &[("raw_orders", "amount")]),
                ],
                Some((0, "B212", 17, 29)),
                is_error,
            ),
            recovery_model(
                "mart",
                "SELECT customer_id, SUM(amount) AS total FROM __ref(\"stg\") WHERE amount > 5 GROUP BY 1",
                &["stg"],
                vec![
                    lineage("customer_id", &[("stg", "customer_id")]),
                    lineage("total", &[("stg", "amount")]),
                ],
                Some((1, "B217", 56, 62)),
                is_error,
            ),
            recovery_model(
                "report",
                "SELECT total AS x FROM __ref(\"mart\")",
                &["mart"],
                vec![lineage("x", &[("mart", "total")])],
                None,
                is_error,
            ),
        ],
        diagnostics: vec![owner(0, "B212", "stg"), owner(1, "B217", "mart")],
    }
}

/// Plan and finish type recovery for one case of the poisoned chain.
pub(crate) fn recovery_summary(test_case: &TypeRecoveryTestCase) -> RecoverySummary {
    let request = poisoned_chain(test_case.dialect, test_case.errors);
    let (status, step) = plan_type_recovery(&request, &catalog()).map_or_else(
        |failure| (format!("{failure:?}"), TypeRecoveryStep::Unchanged),
        |step| (step.status().to_owned(), step),
    );
    let revised = vec![
        test_case
            .revised
            .iter()
            .map(|(code, start, end)| ((*code).to_owned(), *start, *end))
            .collect(),
    ];
    let outcome = step
        .plan()
        .map(|plan| finish_type_recovery(&request, plan, &revised).expect("finishes"))
        .unwrap_or_default();
    (
        status,
        step.plan()
            .map(TypeRecoveryPlan::poisoned_outputs)
            .unwrap_or_default(),
        step.plan()
            .map(TypeRecoveryPlan::revalidated_models)
            .unwrap_or_default(),
        outcome.kept,
        outcome.model_bindings,
    )
}

/// Expected poisoned outputs as owned pairs, in order.
pub(crate) fn expected_poisoned(values: &[(&str, &str)]) -> Vec<(String, String)> {
    shape(values)
}

/// Expected retained diagnostics with owned notes.
pub(crate) fn expected_kept(values: &[(usize, Option<&str>)]) -> Vec<(usize, Option<String>)> {
    values
        .iter()
        .map(|(index, note)| (*index, note.map(str::to_owned)))
        .collect()
}

/// Expected model binding positions as owned vectors.
pub(crate) fn expected_bindings(values: &[&[usize]]) -> Vec<Vec<usize>> {
    values.iter().map(|positions| positions.to_vec()).collect()
}

/// A model diagnostic located at `(line, column)`.
fn model_diagnostic(
    id: usize,
    code: &str,
    message: &str,
    model: &str,
    location: Option<(i64, i64)>,
) -> SemanticDiagnostic {
    SemanticDiagnostic {
        id,
        code: code.to_owned(),
        message: message.to_owned(),
        resource_type: Some("model".to_owned()),
        resource_name: Some(model.to_owned()),
        line: location.map(|(line, _)| line),
        column: location.map(|(_, column)| column),
        location: location.map(|(line, column)| SemanticLocation {
            line,
            column,
            end_line: None,
            end_column: None,
        }),
        help: None,
        notes: Vec::new(),
    }
}

/// A model with its authored SQL behind a MODEL header.
fn completion_model(
    name: &str,
    query_sql: &str,
    inferred: &[&str],
    lineage: Vec<LineageOutput>,
    binding_codes: &[&str],
) -> CompletionModel {
    CompletionModel {
        name: name.to_owned(),
        query_sql: query_sql.to_owned(),
        authored_sql: format!("MODEL (\n  description \"{name}\",\n);\n\n{query_sql}\n"),
        inferred_columns: strings(inferred),
        lineage,
        binding_codes: strings(binding_codes),
        rejected_opt_out_file: None,
    }
}

/// A staging typo poisoning a mart use, a temporal comparison, a rejected opt-out and a test error.
fn project_request() -> CompletionRequest {
    let mut legacy = completion_model(
        "legacy",
        "SELECT mystery(order_id) AS m FROM __ref(\"stg\")",
        &["m"],
        Vec::new(),
        &["B101"],
    );
    legacy.rejected_opt_out_file = Some("legacy.sql".to_owned());
    let mut sql_test = model_diagnostic(
        4,
        "B002",
        "Unknown column 'x' in table 'stg'",
        "report",
        None,
    );
    sql_test.resource_type = Some("sql_test".to_owned());
    CompletionRequest {
        dialect: Some("duckdb".to_owned()),
        diagnostics: vec![
            model_diagnostic(
                0,
                "B002",
                "Unknown column 'amonut' in table 'raw_orders'",
                "stg",
                Some((5, 18)),
            ),
            model_diagnostic(
                1,
                "B002",
                "Unknown column 'amount' in table 'stg'",
                "mart",
                Some((5, 24)),
            ),
            model_diagnostic(
                2,
                "B217",
                "Cannot compare timestamp and integer (context: o.ordered_at > 5)",
                "mart",
                Some((7, 7)),
            ),
            model_diagnostic(
                3,
                "B101",
                "Unknown function 'mystery'",
                "legacy",
                Some((5, 8)),
            ),
            sql_test,
        ],
        models: vec![
            completion_model(
                "stg",
                "SELECT order_id, amonut, ordered_at FROM __source(\"raw_orders\")",
                &["order_id", "amonut", "ordered_at"],
                vec![lineage("amonut", &[("raw_orders", "amonut")])],
                &["B002"],
            ),
            completion_model(
                "mart",
                "SELECT customer_id, SUM(amount) AS total\nFROM __ref(\"stg\") AS o\n\
                 WHERE o.ordered_at > 5\nGROUP BY 1",
                &["customer_id", "total"],
                vec![lineage("total", &[("stg", "amount")])],
                &["B002", "B217"],
            ),
            legacy,
        ],
        shapes: vec![
            (
                "raw_orders".to_owned(),
                shape(&[
                    ("order_id", "INTEGER"),
                    ("customer_id", "INTEGER"),
                    ("amount", "DOUBLE"),
                    ("status", "VARCHAR"),
                ]),
            ),
            (
                "stg".to_owned(),
                shape(&[
                    ("order_id", "INTEGER"),
                    ("amount", "UNKNOWN"),
                    ("ordered_at", "TIMESTAMP"),
                ]),
            ),
        ],
    }
}

/// One expected diagnostic of the completion project.
pub(crate) fn described(
    code: &str,
    message: &str,
    help: Option<&str>,
    notes: &[&str],
    location: Option<(i64, i64, Option<i64>, Option<i64>)>,
) -> DescribedDiagnostic {
    (
        code.to_owned(),
        message.to_owned(),
        help.map(str::to_owned),
        strings(notes),
        location,
    )
}

/// Complete one variant of the project request.
pub(crate) fn completion_summary(test_case: &CompletionTestCase) -> CompletionSummary {
    let mut request = project_request();
    request.models[1]
        .authored_sql
        .push_str(["", "-- caf\u{e9}\n"][usize::from(test_case.non_ascii_comment)]);
    let kept_diagnostics =
        [request.diagnostics.len(), 0][usize::from(test_case.without_diagnostics)];
    request.diagnostics.truncate(kept_diagnostics);
    request.models[2].rejected_opt_out_file =
        [Some("legacy.sql".to_owned()), None][usize::from(test_case.without_diagnostics)].clone();
    let (failure, (diagnostics, model_bindings, order)) =
        complete_semantic_diagnostics(&request, &catalog()).map_or_else(
            |failure| (Some(failure), Default::default()),
            |outcome| (None, outcome.into_parts()),
        );
    let described_order: Vec<DescribedDiagnostic> = order
        .into_iter()
        .map(|entry| described_entry(&request, &diagnostics, entry))
        .collect();
    (failure, described_order, model_bindings)
}

fn described_entry(
    request: &CompletionRequest,
    diagnostics: &[CompletedDiagnostic],
    entry: FinalDiagnostic,
) -> DescribedDiagnostic {
    let (position, opt_out) = entry.into_parts();
    let completed = position.map(|position| described_completed(request, &diagnostics[position]));
    let rejected = opt_out.map(|opt_out| {
        (
            "P009".to_owned(),
            opt_out.message,
            Some(opt_out.help),
            vec![opt_out.note],
            None,
        )
    });
    completed
        .or(rejected)
        .expect("one side of an order entry is set")
}

fn described_completed(
    request: &CompletionRequest,
    diagnostic: &CompletedDiagnostic,
) -> DescribedDiagnostic {
    (
        request.diagnostics[diagnostic.source].code.clone(),
        diagnostic.message.clone(),
        diagnostic.help.clone(),
        diagnostic.notes.clone(),
        diagnostic.location.map(|location| {
            (
                location.line,
                location.column,
                location.end_line,
                location.end_column,
            )
        }),
    )
}

/// Explain a B212 and a B217 on unqualified operands of a mart reading `stg`.
pub(crate) fn operand_type_summary(test_case: &OperandTypeTestCase) -> CompletionSummary {
    let query_sql = format!(
        "SELECT order_id, ordered_at + 1 AS shifted\nFROM __ref(\"stg\"){}\nWHERE ordered_at > 5",
        test_case.joined
    );
    let request = CompletionRequest {
        dialect: Some("duckdb".to_owned()),
        diagnostics: vec![
            model_diagnostic(
                0,
                "B212",
                "Arithmetic operation expects NUMERIC-compatible operands, found TIMESTAMP and INTEGER",
                "mart",
                Some((5, 18)),
            ),
            model_diagnostic(
                1,
                "B217",
                "Incompatible comparison between TIMESTAMP and INTEGER",
                "mart",
                Some((7, 7)),
            ),
        ],
        models: vec![completion_model(
            "mart",
            &query_sql,
            &["order_id", "shifted"],
            Vec::new(),
            &["B212", "B217"],
        )],
        shapes: test_case
            .shapes
            .iter()
            .map(|(name, columns)| ((*name).to_owned(), shape(columns)))
            .collect(),
    };
    let (failure, (diagnostics, model_bindings, order)) =
        complete_semantic_diagnostics(&request, &catalog()).map_or_else(
            |failure| (Some(failure), Default::default()),
            |outcome| (None, outcome.into_parts()),
        );
    let described_order: Vec<DescribedDiagnostic> = order
        .into_iter()
        .map(|entry| described_entry(&request, &diagnostics, entry))
        .collect();
    (failure, described_order, model_bindings)
}

/// The type Python infers for the first projection of `SELECT {expression}` on DuckDB.
pub(crate) fn projected_type(expression: &str) -> Option<String> {
    let parsed = parsed_model(&format!("SELECT {expression}"), Some("duckdb"))
        .expect("duckdb parses")
        .expect("the projection parses");
    let projection = parsed
        .children()
        .into_iter()
        .next()
        .expect("one projection");
    let return_types: HashMap<String, String> =
        HashMap::from([("MY_FN".to_owned(), "DATE".to_owned())]);
    expression_type(projection, &return_types).expect("ASCII expressions are typed")
}

fn metadata_model(name: &str, query_sql: &str, checked: bool) -> MetadataModel {
    MetadataModel {
        name: name.to_owned(),
        query_sql: query_sql.to_owned(),
        authored_sql: format!("MODEL (\n  description \"{name}\",\n);\n\n{query_sql}\n"),
        checked,
        calls_functions: true,
        references: vec![
            ("unique_key".to_owned(), strings(&["order_id"])),
            ("cursor".to_owned(), strings(&["label"])),
            (
                "row_diff_exclude_columns".to_owned(),
                strings(&["missing_col"]),
            ),
        ],
        cursor_inputs: vec![
            (
                "raw_orders".to_owned(),
                strings(&["updated_at", "created_at"]),
            ),
            ("unknown_upstream".to_owned(), strings(&["x"])),
        ],
        cursor: Some("label".to_owned()),
        cursor_type: Some("timestamp".to_owned()),
        audits: Vec::new(),
    }
}

/// A mart calling declared functions badly, with config, source and SQL test metadata.
pub(crate) fn metadata_request(test_case: &MetadataTestCase) -> MetadataRequest {
    let query_sql = "SELECT __udf(\"scaled_amount\")(o.amount, 2) AS scaled,\n  \
                     __udf(\"scaled_amount\")(o.status, 'x') AS wrong,\n  \
                     __udf(\"label_status\")(o.status, 1) AS label\nFROM __ref(\"stg\") AS o";
    MetadataRequest {
        dialect: Some(test_case.dialect.to_owned()),
        quoted_ignore_case: false,
        return_types: Vec::new(),
        functions: vec![
            MetadataFunction {
                key: "scaled_amount".to_owned(),
                arguments: shape(&[("p_amount", "DOUBLE"), ("p_factor", "INTEGER")]),
            },
            MetadataFunction {
                key: "label_status".to_owned(),
                arguments: shape(&[("p_status", "VARCHAR")]),
            },
        ],
        shapes: vec![
            (
                "stg".to_owned(),
                shape(&[
                    ("order_id", "INTEGER"),
                    ("amount", "DOUBLE"),
                    ("status", "VARCHAR"),
                ]),
            ),
            (
                "raw_orders".to_owned(),
                shape(&[("order_id", "INTEGER"), ("updated_at", "TIMESTAMP")]),
            ),
            ("mart".to_owned(), shape(test_case.mart_shape)),
        ],
        models: vec![
            metadata_model("mart", query_sql, true),
            metadata_model("skipped", query_sql, false),
        ],
        sources: vec![MetadataSource {
            name: "raw_orders".to_owned(),
            cursor_column: Some("loaded_at".to_owned()),
            contents: "sources:\n  - name: raw_orders\n    cursor_column: loaded_at\n".into(),
        }],
        sql_tests: vec![MetadataSqlTest {
            contents: "-- test\nWITH __ref__stg AS (SELECT 1 AS order_id, 2 AS refund)\n".into(),
            ctes: vec![
                ("__ref__stg".to_owned(), strings(&["order_id", "refund"])),
                ("__expected__mart".to_owned(), strings(&["scaled", "LABEL"])),
                ("helper".to_owned(), strings(&["zzz"])),
            ],
        }],
    }
}

/// One expected located metadata error.
pub(crate) fn metadata_finding(
    code: &'static str,
    message: &str,
    line: i64,
    column: i64,
) -> MetadataFinding {
    MetadataFinding {
        code,
        message: message.to_owned(),
        line,
        column,
    }
}

/// Run the metadata checks on a fresh catalog.
pub(crate) fn metadata_outcome(
    test_case: &MetadataTestCase,
) -> Result<MetadataOutcome, SemanticFailure> {
    check_semantic_metadata(&metadata_request(test_case), &catalog())
}

/// The findings Python reports for the checked mart of `metadata_request`.
pub(crate) fn expected_checked_outcome() -> MetadataOutcome {
    MetadataOutcome {
        models: vec![
            ModelMetadataFindings {
                functions: vec![
                    metadata_finding(
                        "B301",
                        "Function 'scaled_amount' argument 'p_amount' expects DOUBLE, received \
                         VARCHAR; convert the argument explicitly",
                        5,
                        15,
                    ),
                    metadata_finding(
                        "B301",
                        "Function 'scaled_amount' argument 'p_factor' expects INTEGER, received \
                         VARCHAR; convert the argument explicitly",
                        5,
                        15,
                    ),
                    metadata_finding(
                        "B102",
                        "Function 'label_status' expects 1 arguments but received 2",
                        7,
                        10,
                    ),
                ],
                audits: Vec::new(),
                references: vec![
                    metadata_finding(
                        "B300",
                        "row_diff_exclude_columns references unknown column 'missing_col'",
                        1,
                        1,
                    ),
                    metadata_finding(
                        "B300",
                        "cursor_inputs raw_orders references unknown column 'created_at'",
                        1,
                        1,
                    ),
                    metadata_finding(
                        "B301",
                        "cursor_type timestamp does not match column 'label' type VARCHAR",
                        7,
                        41,
                    ),
                ],
            },
            ModelMetadataFindings::default(),
        ],
        sources: vec![(
            0,
            metadata_finding(
                "B300",
                "cursor_column references unknown column 'loaded_at'",
                3,
                20,
            ),
        )],
        sql_tests: vec![(
            0,
            metadata_finding(
                "B302",
                "SQL test '__ref__stg' names unknown column 'refund'",
                2,
                48,
            ),
            54,
        )],
        fallback_types: Vec::new(),
    }
}

/// A request whose `orders` model outputs only `order_id` and declares audits in its header.
pub(crate) fn missing_column_request(audit: AuditSqlInput) -> ResourceSqlRequest {
    ResourceSqlRequest {
        dialect: Some("duckdb".to_owned()),
        quoted_ignore_case: false,
        require_sql_analysis: false,
        shapes: vec![(
            "orders".to_owned(),
            vec![("order_id".to_owned(), "INTEGER".to_owned())],
        )],
        audits: vec![audit],
        sql_tests: Vec::new(),
        models: vec![HookModelInput {
            name: "orders".to_owned(),
            path: "sql/models/orders.sql".to_owned(),
            authored_sql: Arc::from(
                "MODEL (description \"Orders not_null\", columns (order_id (audits [unique]), \
                 shipped_at (audits [not_null, unique])));\nSELECT 1 AS order_id\n",
            ),
            analysis_disabled: false,
            hooks: Vec::new(),
        }],
        hook_files: Vec::new(),
    }
}
