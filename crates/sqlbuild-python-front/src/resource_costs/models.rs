//! Public records of resource costs.

use crate::native_host::constants::ROWS_PER_STATEMENT;
use crate::native_host::main::execute::execute;
use crate::native_host::models::Location;
use crate::resource_costs::_helpers::resource_costs_state::observation_cells;
use crate::resource_costs::_helpers::resource_costs_state::run_attempt;
use crate::resource_costs::_helpers::resource_costs_state::table;
use crate::resource_costs::_helpers::resource_costs_state::{
    ADD_COLUMNS_METHOD, CREATE_TABLE_METHOD, TABLES_WITH_ADDED_COLUMNS,
};
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyFloat;
use pyo3::types::PyString;
use sqlbuild_core::constants::RESOURCE_COSTS_TABLE_NAME;
use sqlbuild_core::cost_report::models::CostSummary;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_state::resource_costs::constants::RESOURCE_COST_COLUMN_TYPES;
use sqlbuild_state::resource_costs::main::latest_run_costs_sql::latest_run_costs_sql;
use sqlbuild_state::resource_costs::models::RunCostAttempt;
use sqlbuild_state::state_tables::main::values_insert_sql::values_insert_sql;

/// The run a collection belongs to, as the run row records it.
#[derive(Debug)]
pub struct CostRunIdentity<'a> {
    pub run_id: &'a str,
    pub orchestration_run_id: Option<&'a str>,
    pub target_name: Option<&'a str>,
    pub started_at: &'a str,
    pub completed_at: &'a str,
}

/// A target's resource costs table, open on one connection for one build's publication.
#[derive(Debug)]
pub struct ResourceCostStore<'py> {
    pub(crate) finalization: Bound<'py, PyAny>,
    pub(crate) adapter: Bound<'py, PyAny>,
    pub(crate) connection: Bound<'py, PyAny>,
    pub(crate) location: Location,
    pub(crate) table: String,
}

impl<'py> ResourceCostStore<'py> {
    /// Connect when the adapter keeps resource costs and the target has a schema.
    pub fn open(finalization: &Bound<'py, PyAny>) -> PyResult<Option<Self>> {
        let adapter: Bound<'py, PyAny> = native_field(finalization, "adapter")?;
        if !adapter.hasattr(CREATE_TABLE_METHOD)? {
            return Ok(None);
        }
        let Some(schema) =
            native_field(finalization, "target_schema")?.extract::<Option<String>>()?
        else {
            return Ok(None);
        };
        let location = Location {
            database: native_field(finalization, "target_database")?.extract()?,
            schema,
        };
        let Some(table) = table(&adapter, &location)? else {
            return Ok(None);
        };
        let connection: Bound<'py, PyAny> = native_field(&adapter, "connect")?
            .call1((native_field(finalization, "connection_config")?,))?;
        Ok(Some(Self {
            finalization: finalization.clone(),
            adapter,
            connection,
            location,
            table,
        }))
    }

    /// Create the schema and table if missing, then append one collection attempt.
    pub fn append(&self, run: (&CostRunIdentity<'_>, i64), summary: &CostSummary) -> PyResult<()> {
        let py: Python<'py> = self.adapter.py();
        let keywords = self.location.keywords(py);
        for statement in call_with(
            &native_field(&self.adapter, "render_create_schema")?,
            &keywords,
        )?
        .try_iter()?
        {
            self.execute(&statement?.extract::<String>()?)?;
        }
        let create: String = call_with(
            &native_field(&self.adapter, CREATE_TABLE_METHOD)?,
            &keywords,
        )?
        .extract()?;
        if !create.is_empty() {
            self.execute(&create)?;
        }
        self.add_columns(&keywords)?;
        let observed_at: String = call_with(
            &native_field(&self.adapter, "render_current_utc_timestamp")?,
            &[],
        )?
        .extract()?;
        let columns: Vec<&str> = RESOURCE_COST_COLUMN_TYPES
            .iter()
            .map(|(column, _)| *column)
            .collect();
        let cells: Vec<Vec<String>> = observation_cells(py, run, summary, &observed_at)?;
        for chunk in cells.chunks(ROWS_PER_STATEMENT) {
            self.execute(&values_insert_sql(&self.table, &columns, chunk))?;
        }
        Ok(())
    }

    /// The newest run row of each refreshable run; none when the table is missing.
    pub fn latest(&self) -> PyResult<Vec<RunCostAttempt>> {
        let py: Python<'py> = self.adapter.py();
        let mut keywords = self.location.keywords(py);
        keywords.push(("connection", self.connection.clone()));
        keywords.push((
            "name",
            PyString::new(py, RESOURCE_COSTS_TABLE_NAME).into_any(),
        ));
        if !call_with(&native_field(&self.adapter, "get_columns")?, &keywords)?.is_truthy()? {
            return Ok(Vec::new());
        }
        let result: Bound<'py, PyAny> = self.execute(&latest_run_costs_sql(&self.table))?;
        call_with(&native_field(&result, "fetchall")?, &[])?
            .try_iter()?
            .map(|row| run_attempt(&row?))
            .collect()
    }

    /// Re-collect one older run through the adapter, priced at `rate`, within `budget_seconds`.
    pub fn recollect(
        &self,
        run: &RunCostAttempt,
        (usd_per_credit, rate_source): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
        budget_seconds: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = self.adapter.py();
        let parse = imported(py, "datetime", "datetime")?.getattr("fromisoformat")?;
        let ledger: Bound<'py, PyAny> = native_field(&self.finalization, "project_dir")?
            .call_method1("__truediv__", ("target",))?
            .call_method1("__truediv__", ("executions",))?
            .call_method1("__truediv__", (run.run_id.as_str(),))?
            .call_method1("__truediv__", ("statements.jsonl",))?;
        call_with(
            &native_field(&self.adapter, "collect_run_cost")?,
            &[
                (
                    "connection_config",
                    native_field(&self.finalization, "connection_config")?,
                ),
                (
                    "target_database",
                    native_field(&self.finalization, "target_database")?,
                ),
                ("run_id", PyString::new(py, &run.run_id).into_any()),
                ("started_at", parse.call1((run.run_started_at.as_str(),))?),
                (
                    "completed_at",
                    parse.call1((run.run_completed_at.as_str(),))?,
                ),
                ("statement_ledger_path", ledger),
                ("usd_per_credit", usd_per_credit.clone()),
                ("rate_source", rate_source.clone()),
                (
                    "budget_seconds",
                    PyFloat::new(py, budget_seconds).into_any(),
                ),
            ],
        )
    }

    /// Add the columns an older table lacks, once per table and process.
    fn add_columns(&self, keywords: &[(&str, Bound<'py, PyAny>)]) -> PyResult<()> {
        if !self.adapter.hasattr(ADD_COLUMNS_METHOD)?
            || TABLES_WITH_ADDED_COLUMNS
                .lock()
                .is_ok_and(|done| done.contains(&self.table))
        {
            return Ok(());
        }
        for statement in
            call_with(&native_field(&self.adapter, ADD_COLUMNS_METHOD)?, keywords)?.try_iter()?
        {
            self.execute(&statement?.extract::<String>()?)?;
        }
        if let Ok(mut done) = TABLES_WITH_ADDED_COLUMNS.lock() {
            done.insert(self.table.clone());
        }
        Ok(())
    }

    pub fn close(self) -> PyResult<()> {
        native_field(&self.adapter, "close")?
            .call1((&self.connection,))
            .map(|_| ())
    }

    pub(crate) fn execute(&self, sql: &str) -> PyResult<Bound<'py, PyAny>> {
        execute(&self.adapter, &self.connection, sql)
    }
}
