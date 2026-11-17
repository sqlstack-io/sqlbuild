//! The warehouse reads planning consults, as traits.

use crate::planner_warehouse::models::EligibleCursorQuery;
use crate::planner_warehouse::models::FingerprintRead;
use crate::planner_warehouse::models::Read;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_python_front::native_host::models::Location;
use sqlbuild_python_front::state_rows::models::EventTableLayout;
use sqlbuild_state::consumption::models::ConsumptionRow;

/// Progress of the cursor bound statements a plan runs.
pub trait CursorProgress<'py> {
    /// Before statement `index` runs, when statements run one at a time.
    fn starting(&self, index: usize) -> PyResult<()>;
    /// Before statements run `concurrency` at a time.
    fn concurrent(&self, concurrency: usize) -> PyResult<()>;
    /// After statement `index` ran, with its `(row, seconds, error)` outcome.
    fn finished(&self, index: usize, outcome: &Bound<'py, PyAny>) -> PyResult<()>;
    /// The `on_complete(index=, result=)` callback concurrent statements report through.
    fn completion_callback(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>>;
}

/// Everything a plan reads from the warehouse, live or from a synced state that may answer unknown.
pub trait WarehouseReads: Send + Sync + std::fmt::Debug {
    /// The listed relations of `database` narrowed to `schemas` and `names`, as `RelationInfo`s.
    fn relations<'py>(
        &self,
        py: Python<'py>,
        database: &Bound<'py, PyAny>,
        filter: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<Bound<'py, PyAny>>;

    /// The columns of each listed relation, keyed by `(database, schema, name)`.
    fn columns<'py>(
        &self,
        py: Python<'py>,
        relations: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>>;

    /// The newest fingerprint per node of one state location, as a `FingerprintSet`.
    fn fingerprints<'py>(
        &self,
        py: Python<'py>,
        read: FingerprintRead<'_>,
    ) -> PyResult<Bound<'py, PyAny>>;

    /// Run `plan` with one inspection catalog serving its listings.
    fn within_inspection<'py>(
        &self,
        py: Python<'py>,
        plan: &mut dyn FnMut() -> PyResult<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>>;

    /// How many metadata reads may run at once.
    fn inspection_concurrency(&self, py: Python<'_>) -> PyResult<i64>;

    /// Warm the listings of `(database, schema)` scopes the plan will read, where that helps.
    fn prefetch_schema_listings<'py>(
        &self,
        py: Python<'py>,
        scopes: Bound<'py, PyAny>,
    ) -> PyResult<()>;

    /// Whether listing by schema and name finds relations a name-only listing would.
    fn lists_by_schema(&self, py: Python<'_>) -> PyResult<bool>;

    /// Each statement's `(row, seconds, error)`; `statements` renders SQL only for reads that run it.
    fn cursor_bounds<'py>(
        &self,
        py: Python<'py>,
        statements: &dyn Fn() -> PyResult<Vec<String>>,
        progress: &dyn CursorProgress<'py>,
    ) -> PyResult<Read<Vec<Bound<'py, PyAny>>>>;

    /// Each eligible target cursor query's rows.
    fn eligible_cursor_maxes<'py>(
        &self,
        py: Python<'py>,
        queries: Vec<EligibleCursorQuery>,
    ) -> PyResult<Read<Vec<Bound<'py, PyAny>>>>;

    /// The model's cursor `MAX` text as its newest successful build observed it.
    fn target_max(&self, model: &str) -> Read<String>;

    /// Note that `model` reads from `start` up to the latest data when the run starts.
    fn note_window(&self, model: &str, start: &str);

    /// Note one item the plan reports as unknown until the run plans live.
    fn note_unknown(&self, item: String);

    /// The columns an expression source's `sql` yields.
    fn described_columns(
        &self,
        py: Python<'_>,
        source: &str,
        sql: &str,
    ) -> PyResult<Read<Vec<String>>>;

    /// Whether each relation `relations` renders can be read, as `RelationReadProbe`s in order.
    fn read_access<'py>(
        &self,
        py: Python<'py>,
        relations: &dyn Fn() -> PyResult<Vec<Bound<'py, PyAny>>>,
    ) -> PyResult<Read<Vec<Bound<'py, PyAny>>>>;

    /// The latest recorded source freshness of `(database, schema)`, as a latest-records read.
    fn source_freshness_state<'py>(
        &self,
        py: Python<'py>,
        location: (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
        read: (bool, Option<&[String]>),
    ) -> PyResult<Read<Bound<'py, PyAny>>>;

    /// Each table freshness request's metadata, keyed by request.
    fn table_freshness<'py>(
        &self,
        py: Python<'py>,
        requests: Bound<'py, PyAny>,
    ) -> PyResult<Read<Bound<'py, PyAny>>>;

    /// One table's freshness metadata.
    fn single_table_freshness<'py>(
        &self,
        py: Python<'py>,
        table: [Bound<'py, PyAny>; 3],
    ) -> PyResult<Read<Bound<'py, PyAny>>>;

    /// The rows of one source freshness query.
    fn freshness_query<'py>(
        &self,
        py: Python<'py>,
        sql: Bound<'py, PyAny>,
    ) -> PyResult<Read<Bound<'py, PyAny>>>;

    /// Whether listed relations carry their table type.
    fn table_types(&self) -> Read<()>;

    /// The callable that reads one retention request's state.
    fn retention<'py>(&self, py: Python<'py>) -> PyResult<Read<Bound<'py, PyAny>>>;

    /// Why `destination` cannot rename columns, `None` when it can.
    fn column_rename_unavailable<'py>(
        &self,
        py: Python<'py>,
        destination: Bound<'py, PyAny>,
    ) -> PyResult<Read<Bound<'py, PyAny>>>;

    /// The migration state tables of `schemas` in `database`, narrowed to `names`.
    fn migration_state_tables<'py>(
        &self,
        py: Python<'py>,
        database: &Bound<'py, PyAny>,
        filter: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<Read<Bound<'py, PyAny>>>;

    /// The recorded migration events of one state table in `(database, schema)`.
    fn recorded_migrations<'py>(
        &self,
        py: Python<'py>,
        layout: &EventTableLayout,
        location: (Bound<'py, PyAny>, Bound<'py, PyAny>),
        stored: Option<&[String]>,
    ) -> PyResult<Read<Bound<'py, PyTuple>>>;

    /// The stored columns of the old-name view table in `(database, schema)`.
    fn old_name_view_columns<'py>(
        &self,
        py: Python<'py>,
        location: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<Read<Vec<String>>>;

    /// Relations of the deferred target's `(database, schema)`, by name; deferral plans live only.
    fn deferred_relations<'py>(
        &self,
        py: Python<'py>,
        location: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<Bound<'py, PyAny>>;

    /// The newest executions per relation recorded in `location`'s consumption table.
    fn recent_consumption(
        &self,
        py: Python<'_>,
        location: &Location,
    ) -> PyResult<Vec<ConsumptionRow>>;

    /// The named executions recorded in `location`'s consumption table.
    fn consumption_executions(
        &self,
        py: Python<'_>,
        location: &Location,
        execution_ids: &[&str],
    ) -> PyResult<Vec<ConsumptionRow>>;

    /// Close what reading consumption history opened.
    fn close_consumption(&self, py: Python<'_>) -> PyResult<()>;
}
