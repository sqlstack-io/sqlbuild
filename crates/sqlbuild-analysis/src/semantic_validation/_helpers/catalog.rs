//! Compile-owned schema catalog and native, batched binding requests.

use crate::semantic_validation::main::normalize_batch;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use crate::semantic_validation::models::{
    CatalogInput, Columns, CompactAnalysisJob, DiagnosticRow, FunctionProbes, ProjectCatalog,
};
use crate::semantic_validation::types::{
    BindingRequest, NormalizationRequest, PreparedCompactAnalysis, Relations,
};
use crate::semantic_validation::{
    _helpers::{diagnostics, identifiers},
    main as validation,
};
use polyglot_sql::validation::{SchemaColumn, SchemaTable};
use polyglot_sql::{
    Dialect, DialectType, SchemaValidationOptions, ValidationError, ValidationResult,
    ValidationSchema,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

const ANALYSIS_WORKERS: usize = 4;

const ANALYSIS_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_BINDING_MESSAGE: &str = "SQL binding failed";

/// Names some projection quotes, as a CTE's `AS "listingId"` read by `SELECT *`, and none leaves bare.
fn nested_quoted_aliases(parsed: &[polyglot_sql::Expression]) -> HashMap<String, String> {
    use polyglot_sql::{Expression, ExpressionWalk};
    let mut quoted: HashMap<String, String> = HashMap::new();
    let mut unquoted: std::collections::HashSet<String> = std::collections::HashSet::new();
    for expression in parsed {
        for node in expression.dfs() {
            let Expression::Select(select) = node else {
                continue;
            };
            for projection in &select.expressions {
                let identifier = match projection {
                    Expression::Alias(alias) => &alias.alias,
                    Expression::Column(column) => &column.name,
                    _ => continue,
                };
                if identifier.quoted {
                    quoted.insert(
                        identifier.name.clone(),
                        format!("\"{}\"", identifier.name.replace('"', "\"\"")),
                    );
                } else {
                    unquoted.insert(identifier.name.clone());
                }
            }
        }
    }
    quoted.retain(|name, _| !unquoted.contains(name));
    quoted
}

impl ProjectCatalog {
    /// Map inferred column types onto the binding names the query projects, in column order.
    pub fn inferred_schema(
        &self,
        sql: &str,
        columns: Columns,
        inputs: Relations,
    ) -> Result<Vec<(String, Option<String>)>, String> {
        use polyglot_sql::{Expression, ExpressionWalk};
        let parsed = Dialect::get(self.dialect)
            .parse(sql)
            .map_err(|error| error.to_string())?;
        let mut identifiers: HashMap<String, String> = HashMap::new();
        for expression in &parsed {
            if let Some(select) = expression.dfs().find_map(|node| match node {
                Expression::Select(select) => Some(select),
                _ => None,
            }) {
                for projection in &select.expressions {
                    let identifier = match projection {
                        Expression::Alias(alias) => Some(&alias.alias),
                        Expression::Column(column) => Some(&column.name),
                        _ => None,
                    };
                    if let Some(identifier) = identifier {
                        let name = if identifier.quoted {
                            format!("\"{}\"", identifier.name.replace('"', "\"\""))
                        } else {
                            identifier.name.clone()
                        };
                        identifiers.insert(identifier.name.clone(), name);
                    }
                }
                break;
            }
        }
        let nested_quoted: HashMap<String, String> = nested_quoted_aliases(&parsed);
        let mut exact_inputs: HashMap<String, String> = HashMap::new();
        for shape in inputs.values() {
            for (name, _) in &shape.0 {
                if name.len() > 1 && name.starts_with('"') && name.ends_with('"') {
                    exact_inputs
                        .insert(name[1..name.len() - 1].replace("\"\"", "\""), name.clone());
                }
            }
        }
        let mut result: Vec<(String, Option<String>)> = Vec::with_capacity(columns.0.len());
        let mut positions: HashMap<String, usize> = HashMap::new();
        for (name, column_type) in columns.0 {
            let binding_name = identifiers
                .get(&name)
                .or_else(|| nested_quoted.get(&name))
                .or_else(|| exact_inputs.get(&name))
                .cloned()
                .unwrap_or(name);
            match positions.get(&binding_name) {
                Some(position) => result[*position].1 = column_type,
                None => {
                    positions.insert(binding_name.clone(), result.len());
                    result.push((binding_name, column_type));
                }
            }
        }
        Ok(result)
    }

    pub fn new(request: CatalogInput) -> Result<Self, String> {
        let CatalogInput {
            dialect,
            quoted_ignore_case,
            known_functions,
            known_types,
            relations,
        } = request;
        let dialect = dialect
            .parse::<DialectType>()
            .map_err(|error| error.to_string())?;
        let mut catalog = Self {
            dialect,
            quoted_ignore_case,
            options: SchemaValidationOptions {
                known_functions,
                known_types,
                check_types: matches!(
                    dialect,
                    DialectType::DuckDB
                        | DialectType::PostgreSQL
                        | DialectType::Snowflake
                        | DialectType::BigQuery
                ),
                check_references: true,
                semantic: true,
                strict: Some(true),
                complexity_guard: Some(polyglot_sql::ComplexityGuardOptions {
                    max_function_call_depth: Some(512),
                    ..Default::default()
                }),
                ..Default::default()
            },
            tables: HashMap::new(),
            overrides: Vec::new(),
            analysis_tables: HashMap::new(),
            function_probes: Arc::new(FunctionProbes::default()),
            analysis_pool: Arc::default(),
            running_analyses: Arc::default(),
        };
        catalog.update_relations(relations);
        Ok(catalog)
    }

    pub fn update_relations(&mut self, relations: Relations) {
        for (name, columns) in relations {
            self.tables.insert(name.clone(), self.table(&name, columns));
        }
    }

    pub fn with_relations(&self, relations: Relations) -> Self {
        let mut catalog = Self {
            dialect: self.dialect,
            options: self.options.clone(),
            quoted_ignore_case: self.quoted_ignore_case,
            tables: self.tables.clone(),
            overrides: Vec::new(),
            analysis_tables: self.analysis_tables.clone(),
            function_probes: Arc::new(FunctionProbes::default()),
            analysis_pool: Arc::clone(&self.analysis_pool),
            running_analyses: Arc::clone(&self.running_analyses),
        };
        catalog.update_relations(relations);
        catalog
    }

    pub fn update_analysis(&mut self, relations: HashMap<String, (Columns, Columns)>) {
        for (name, (types, nullability)) in relations {
            let types: HashMap<_, _> = types.0.into_iter().collect();
            let mut table = self.table(&name, Columns::default());
            table.name = name.clone();
            for (column, nullable) in nullability.0 {
                table.columns.push(SchemaColumn {
                    data_type: types
                        .get(&column)
                        .and_then(Clone::clone)
                        .unwrap_or_else(|| "UNKNOWN".to_owned()),
                    name: column,
                    nullable: match nullable.as_deref() {
                        Some("non_null") => Some(false),
                        Some("nullable") => Some(true),
                        _ => None,
                    },
                    primary_key: false,
                    unique: false,
                    references: None,
                });
            }
            self.analysis_tables.insert(name, table);
        }
    }

    pub fn register_override(&mut self, relations: Relations) -> usize {
        let tables: HashMap<String, SchemaTable> = relations
            .into_iter()
            .map(|(name, columns)| (name.clone(), self.table(&name, columns)))
            .collect();
        self.overrides.push(tables);
        self.overrides.len() - 1
    }

    pub fn validation_payload(&self, requests: Vec<BindingRequest>) -> Result<String, String> {
        let mut payloads = Vec::with_capacity(requests.len());
        for (sql, references, overrides) in requests {
            let schema = self.schema(&references, overrides)?;
            payloads.push(serde_json::json!({"sql": sql, "dialect": self.dialect.to_string(), "schema": schema,
                "options": self.options, "quoted_ignore_case": self.quoted_ignore_case}));
        }
        serde_json::to_string(&payloads).map_err(|error| error.to_string())
    }

    /// Validate each request on its own schema, as `validations_json` does, as diagnostic rows.
    pub fn schema_validation_rows(
        &self,
        dialect: &str,
        requests: Vec<BindingRequest>,
    ) -> Result<Vec<Vec<DiagnosticRow>>, String> {
        let sqls: Vec<String> = requests.iter().map(|request| request.0.clone()).collect();
        let payload: String = self.validation_payload(requests)?;
        let results: Vec<ValidationResult> =
            serde_json::from_str(&validation::validations_json(&payload)?)
                .map_err(|error| error.to_string())?;
        if results.len() != sqls.len() {
            return Err(
                "native SQL schema validation returned an invalid batch response".to_owned(),
            );
        }
        sqls.iter()
            .zip(results)
            .map(|(sql, result)| {
                let rows: Vec<DiagnosticRow> =
                    result.errors.into_iter().map(messaged_row).collect();
                diagnostics::binding_diagnostics(sql, dialect, rows)
            })
            .collect()
    }

    /// Resolve a compact batch now; the job runs later while the catalog keeps changing.
    pub fn prepare_compact(&self, text: &str) -> Result<CompactAnalysisJob, String> {
        let analysis =
            crate::query_analysis::main::prepare_project_catalog::prepare_project_compact_with_catalog(
                text, self,
            )?;
        Ok(CompactAnalysisJob {
            analysis: Mutex::new(Some(analysis)),
            catalog: self.analysis_view(),
        })
    }

    pub fn binding_results(
        &self,
        requests: Vec<BindingRequest>,
    ) -> Result<Vec<Vec<DiagnosticRow>>, String> {
        let pool = self.analysis_pool()?;
        let _running = RunningAnalysis::start(&self.running_analyses);
        pool.install(|| {
            requests
                .into_par_iter()
                .map(|(sql, references, overrides)| {
                    let schema = self.schema(&references, overrides)?;
                    let result = self.validate(&sql, &schema)?;
                    let result = diagnostics::map_diagnostics(
                        &sql,
                        self.dialect,
                        result,
                        &self.function_probes,
                    )?;
                    Ok(result.errors.into_iter().map(diagnostic_row).collect())
                })
                .collect::<Result<Vec<_>, String>>()
        })
    }

    /// Normalize a preparation batch on the idle analysis pool, else in turn.
    pub fn normalize_analysis_sqls(
        &self,
        dialect: &str,
        requests: Vec<NormalizationRequest>,
    ) -> Result<Vec<Result<String, String>>, String> {
        let pool = (self.running_analyses.load(Ordering::Acquire) == 0)
            .then(|| self.analysis_pool())
            .transpose()?;
        Ok(normalize_batch::normalize_analysis_sqls(
            dialect,
            requests,
            pool.as_deref(),
        ))
    }
}

/// Count one analysis batch as running on the catalog's pool until dropped.
struct RunningAnalysis<'a>(&'a AtomicUsize);

impl<'a> RunningAnalysis<'a> {
    fn start(running: &'a AtomicUsize) -> Self {
        running.fetch_add(1, Ordering::AcqRel);
        Self(running)
    }
}

impl Drop for RunningAnalysis<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl CompactAnalysisJob {
    /// Take the resolved analysis; a job runs at most once.
    pub fn take_analysis(&self) -> Option<PreparedCompactAnalysis> {
        self.analysis
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    /// Run a taken analysis, counted as running on the catalog's pool.
    pub fn run(&self, analysis: PreparedCompactAnalysis) -> Result<String, String> {
        let _running = RunningAnalysis::start(&self.catalog.running_analyses);
        analysis(&self.catalog)
    }
}

impl ProjectCatalog {
    /// Return the options-only view a resolved analysis batch validates with.
    pub(crate) fn analysis_view(&self) -> Self {
        Self {
            dialect: self.dialect,
            options: self.options.clone(),
            quoted_ignore_case: self.quoted_ignore_case,
            tables: HashMap::new(),
            overrides: Vec::new(),
            analysis_tables: HashMap::new(),
            function_probes: Arc::clone(&self.function_probes),
            analysis_pool: Arc::clone(&self.analysis_pool),
            running_analyses: Arc::clone(&self.running_analyses),
        }
    }

    /// Return this compile's analysis pool, building it on first use.
    pub(crate) fn analysis_pool(&self) -> Result<Arc<rayon::ThreadPool>, String> {
        let mut pool = self
            .analysis_pool
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(pool) = pool.as_ref() {
            return Ok(Arc::clone(pool));
        }
        let built = Arc::new(new_analysis_pool()?);
        *pool = Some(Arc::clone(&built));
        Ok(built)
    }

    pub(crate) fn with_options(
        dialect: DialectType,
        options: SchemaValidationOptions,
        quoted_ignore_case: bool,
    ) -> Self {
        Self {
            dialect,
            options,
            quoted_ignore_case,
            tables: HashMap::new(),
            overrides: Vec::new(),
            analysis_tables: HashMap::new(),
            function_probes: Arc::new(FunctionProbes::default()),
            analysis_pool: Arc::default(),
            running_analyses: Arc::default(),
        }
    }
    pub(crate) fn analysis_schema(
        &self,
        references: &[(String, String)],
    ) -> Option<ValidationSchema> {
        let mut tables: Vec<SchemaTable> = Vec::new();
        for (name, alias) in references {
            if let Some(table) = self
                .analysis_tables
                .get(name)
                .filter(|table| !table.columns.is_empty())
            {
                let mut table = table.clone();
                table.name = alias.clone();
                tables.push(table);
            }
        }
        if tables.is_empty() {
            None
        } else {
            Some(ValidationSchema {
                tables,
                strict: None,
            })
        }
    }
    pub(crate) fn reference_schema(
        &self,
        references: &[(String, bool)],
        override_id: Option<usize>,
    ) -> Result<ValidationSchema, String> {
        let mut schema = self.schema(references, HashMap::new())?;
        if let Some(id) = override_id {
            let overrides = self
                .overrides
                .get(id)
                .ok_or("invalid native binding override")?;
            for ((name, _), table) in references.iter().zip(&mut schema.tables) {
                if let Some(replacement) = overrides.get(name) {
                    *table = replacement.clone();
                }
            }
        }
        Ok(schema)
    }
    /// Rename `reference_schema` tables to the relation stubs of canonical SQL.
    pub(crate) fn alias_reference_schema(
        &self,
        schema: &mut ValidationSchema,
        references: &[(String, bool)],
        aliases: &HashMap<String, String>,
    ) {
        for ((name, _), table) in references.iter().zip(&mut schema.tables) {
            if let Some(alias) = aliases.get(name) {
                table.name = self.schema_name(alias.clone());
            }
        }
    }

    fn table(&self, name: &str, columns: Columns) -> SchemaTable {
        let columns: Vec<_> = columns
            .0
            .into_iter()
            .map(|(name, data_type)| SchemaColumn {
                name: self.schema_name(name),
                data_type: data_type.unwrap_or_else(|| "UNKNOWN".to_owned()),
                nullable: None,
                primary_key: false,
                unique: false,
                references: None,
            })
            .collect();
        SchemaTable {
            name: self.schema_name(name.to_owned()),
            schema: None,
            columns,
            aliases: Vec::new(),
            primary_key: Vec::new(),
            unique_keys: Vec::new(),
            foreign_keys: Vec::new(),
        }
    }

    fn schema_name(&self, name: String) -> String {
        if self.quoted_ignore_case {
            name.to_ascii_uppercase()
        } else {
            name
        }
    }

    pub(crate) fn schema(
        &self,
        references: &[(String, bool)],
        mut overrides: Relations,
    ) -> Result<ValidationSchema, String> {
        let mut tables = Vec::with_capacity(references.len());
        for (name, closed) in references {
            let table = if let Some(columns) = overrides.remove(name) {
                self.table(name, columns)
            } else if !closed {
                self.table(name, Columns::default())
            } else {
                self.tables
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("binding catalog has no relation {name}"))?
            };
            tables.push(table);
        }
        Ok(ValidationSchema {
            tables,
            strict: Some(true),
        })
    }

    pub(crate) fn validate(
        &self,
        sql: &str,
        schema: &ValidationSchema,
    ) -> Result<ValidationResult, String> {
        self.statement_validation(sql, schema, None)
    }

    pub(crate) fn expression_validation(
        &self,
        sql: &str,
        schema: &ValidationSchema,
        expression: polyglot_sql::Expression,
    ) -> Result<ValidationResult, String> {
        self.statement_validation(sql, schema, Some(vec![expression]))
    }

    pub(crate) fn needs_identifier_encoding(&self, sql: &str, schema: &ValidationSchema) -> bool {
        if self.quoted_ignore_case {
            sql.contains('"')
        } else {
            schema.tables.iter().any(has_exact_columns)
        }
    }

    fn statement_validation(
        &self,
        sql: &str,
        schema: &ValidationSchema,
        parsed: Option<Vec<polyglot_sql::Expression>>,
    ) -> Result<ValidationResult, String> {
        let exact_names = !self.quoted_ignore_case && schema.tables.iter().any(has_exact_columns);
        let mut encoded_schema;
        let schema = if exact_names {
            encoded_schema = schema.clone();
            for table in &mut encoded_schema.tables {
                table.name = identifiers::encoded_name(&table.name, self.dialect);
                if let Some(name) = &mut table.schema {
                    *name = identifiers::encoded_name(name, self.dialect);
                }
                for name in &mut table.aliases {
                    *name = identifiers::encoded_name(name, self.dialect);
                }
                for column in &mut table.columns {
                    column.name = identifiers::encoded_name(&column.name, self.dialect);
                }
            }
            &encoded_schema
        } else {
            schema
        };
        let statements = match parsed.map_or_else(
            || {
                Dialect::get(self.dialect).parse_with_options(
                    sql,
                    &polyglot_sql::ParseOptions {
                        complexity_guard: self.options.complexity_guard,
                    },
                )
            },
            Ok,
        ) {
            Ok(statements) => statements,
            Err(_) => {
                return Ok(polyglot_sql::validate_with_schema(
                    sql,
                    self.dialect,
                    schema,
                    &self.options,
                ));
            }
        };
        if statements.iter().any(|statement| !statement.is_statement()) {
            return Ok(polyglot_sql::validate_with_schema(
                sql,
                self.dialect,
                schema,
                &self.options,
            ));
        }
        let (statements, authored) = if exact_names {
            identifiers::transform_statements(statements, Some(self.dialect))?
        } else if self.quoted_ignore_case && sql.contains('"') {
            identifiers::fold_statements(statements)?
        } else {
            (statements, Vec::new())
        };
        let mut clause_errors: Vec<ValidationError> = Vec::new();
        if validation::may_have_extra_clause_checks(sql) {
            let mapping = polyglot_sql::mapping_schema_from_validation_schema_with_dialect(
                schema,
                self.dialect,
            );
            for statement in &statements {
                clause_errors.extend(validation::missing_clause_columns(
                    &polyglot_sql::build_scope(statement),
                    &mapping,
                ));
            }
        }
        let mut result = polyglot_sql::validation::validate_parsed_with_schema(
            statements,
            self.dialect,
            schema,
            &self.options,
        );
        if result.valid {
            result.errors.extend(clause_errors);
        }
        for error in &mut result.errors {
            if exact_names {
                error.message =
                    identifiers::restore_names(&error.message, &authored, error.start, error.end)?;
                continue;
            }
            for (normalized, spelling, span) in &authored {
                if error.start.is_none_or(|start| {
                    span.is_none_or(|span| {
                        start <= span.end && error.end.unwrap_or(start) >= span.start
                    })
                }) {
                    error.message = error
                        .message
                        .replace(&format!("'{normalized}'"), &format!("'{spelling}'"))
                        .replace(
                            &format!("'{}'", normalized.to_ascii_lowercase()),
                            &format!("'{spelling}'"),
                        );
                }
            }
        }
        result.valid = !result
            .errors
            .iter()
            .any(|error| error.severity == polyglot_sql::ValidationSeverity::Error);
        Ok(result)
    }
}

/// The diagnostic row of `error`, naming the failure when the parser gives no message.
fn messaged_row(error: ValidationError) -> DiagnosticRow {
    let mut row: DiagnosticRow = diagnostic_row(error);
    if row.message.is_empty() {
        DEFAULT_BINDING_MESSAGE.clone_into(&mut row.message);
    }
    row
}

pub(super) fn diagnostic_row(error: ValidationError) -> DiagnosticRow {
    let severity = match error.severity {
        polyglot_sql::ValidationSeverity::Error => "error",
        _ => "warning",
    };
    DiagnosticRow {
        code: error.code,
        message: error.message,
        line: error.line,
        column: error.column,
        start: error.start,
        end: error.end,
        severity: severity.to_owned(),
    }
}

fn has_exact_columns(table: &SchemaTable) -> bool {
    table
        .columns
        .iter()
        .any(|column| column.name.starts_with('"'))
}

/// A deep-stack analysis pool, for work that runs without a compile's catalog.
pub(crate) fn new_analysis_pool() -> Result<rayon::ThreadPool, String> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(ANALYSIS_WORKERS)
        .stack_size(ANALYSIS_WORKER_STACK_BYTES)
        .thread_name(|index| format!("sqlbuild-analysis-{index}"))
        .build()
        .map_err(|error| error.to_string())
}
