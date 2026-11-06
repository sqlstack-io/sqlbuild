"""Strict adapter requiring full implementation of every method."""

from __future__ import annotations

from abc import abstractmethod
from decimal import Decimal
from typing import Any, ClassVar

from sqlbuild._native import SourceFreshnessRecord, SqlValue
from sqlbuild.adapter.contract.classes.connection import ConnectionMixin
from sqlbuild.adapter.contract.classes.diff import DiffMixin
from sqlbuild.adapter.contract.classes.materialization import MaterializationMixin
from sqlbuild.adapter.contract.classes.schema import SchemaMixin
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.models import (
    ColumnInfo,
    CursorValue,
    ExpressionInferenceProfile,
    FunctionDefinition,
    MigrationStagePlan,
    RelationGrant,
    RelationInfo,
    RelationReadProbe,
    RelationUsageReport,
    RowDiffTolerance,
    RowDiffTolerances,
    SnapshotChangeTarget,
    TableFreshnessMetadata,
    TableFreshnessRequest,
    TemporaryFunction,
)
from sqlbuild.adapter.contract.types import (
    FrameworkType,
    LoaderLogicalType,
    PromotionStrategy,
    StatementSizeLimit,
    TablePromotionMode,
)
from sqlbuild.compiler.compile.types import FunctionLanguage


class StrictAdapter(
    ConnectionMixin,
    SchemaMixin,
    MaterializationMixin,
    DiffMixin,
):
    """All-abstract adapter interface."""

    adapter_name: ClassVar[str]

    @abstractmethod
    def widen_target_nullability(
        self,
        *,
        connection: Any,
        destination: str,
        target_columns: tuple[ColumnInfo, ...],
        incoming_columns: tuple[ColumnInfo, ...],
        statement_recorder: StatementRecorder,
    ) -> tuple[ColumnInfo, ...]:
        """Let target columns admit NULL where incoming ones do; return the target columns."""
        ...

    @abstractmethod
    def supports_zero_copy_clone(self) -> bool:
        """Return whether clone operations can use zero-copy semantics."""
        ...

    @abstractmethod
    def supports_durable_clone(self) -> bool:
        """Return whether the adapter has a native durable clone primitive."""
        ...

    @abstractmethod
    def supports_relation_age_metadata(self) -> bool:
        """Return whether relation metadata includes reliable age information."""
        ...

    @abstractmethod
    def supports_table_freshness_metadata(self) -> bool:
        """Return whether physical table metadata can provide source freshness."""
        ...

    @abstractmethod
    def supports_python_functions(self) -> bool:
        """Return whether the adapter can create Python UDF resources."""
        ...

    @abstractmethod
    def persists_python_functions(self) -> bool:
        """Return whether Python UDF resources survive across connections."""
        ...

    @abstractmethod
    def python_functions_inherit_default_namespace(self) -> bool:
        """Return whether Python UDFs inherit default database/schema targets."""
        ...

    @abstractmethod
    def supports_unqualified_function_fingerprints(self) -> bool:
        """Return whether unqualified functions can store fingerprints elsewhere."""
        ...

    @abstractmethod
    def supports_table_functions(self) -> bool:
        """Return whether the adapter can create table function resources."""
        ...

    @abstractmethod
    def max_statement_size(self) -> StatementSizeLimit | None:
        """Return the hard SQL text limit and unit, or None; bytes mean UTF-8."""
        ...

    @abstractmethod
    def maximum_identifier_length(self) -> int:
        """Return the maximum unqualified identifier length supported by the adapter."""
        ...

    @abstractmethod
    def interrupt_connection(self, connection: Any) -> bool:
        """Cancel a connection's running statement from another thread; False if unsupported."""
        ...

    def create_sql_test_output(self, *, connection: Any, sql: str, name: str) -> str | None:
        """Optional: a relation holding a unit test's output, or `None` to compare inline."""

        del connection, sql, name
        return None

    def drop_sql_test_output(self, *, connection: Any, relation: str) -> None:
        """Optional: remove a relation `create_sql_test_output` returned."""

        del connection, relation

    def drop_table(
        self, *, connection: Any, destination: str, statement_recorder: StatementRecorder
    ) -> None:
        """Optional: drop a staging or delta table SQLBuild created, if it exists."""

        self.drop(
            connection=connection,
            destination=destination,
            if_exists=True,
            statement_recorder=statement_recorder,
        )

    def list_relations_with_prefix(
        self, *, connection: Any, database: str | None, schemas: tuple[str, ...], prefix: str
    ) -> tuple[RelationInfo, ...] | None:
        """Optional: relations in ``schemas`` whose names start with ``prefix``, or None."""

        del connection, database, schemas, prefix
        return None

    def create_table_as_replaces(self) -> bool:
        """Optional: whether `create_table_as` replaces a table already at its destination."""

        return False

    def render_cursor_operand(self, *, expression: str, cursor_type: str | None) -> str:
        """Optional: the cursor-column expression compared with cursor bounds and aggregated."""

        del cursor_type
        return expression

    def render_cursor_text(self, *, expression: str, cursor_type: str) -> str:
        """Optional: a cursor bound expression as the text the planner parses as a cursor value."""

        del cursor_type
        return f"CAST({expression} AS VARCHAR)"

    def render_create_temporary_function(
        self, *, name: str, definition: FunctionDefinition
    ) -> TemporaryFunction | None:
        """Optional: a session-scoped function copy for unit tests, or `None` if unsupported."""

        del name, definition
        return None

    @abstractmethod
    def lists_relations_authoritatively(self, *, database: str | None, schema: str) -> bool:
        """Return whether ``list_relations`` reports every relation in this scope."""
        ...

    @abstractmethod
    def supports_concurrent_microbatch_dml(self) -> bool:
        """Return whether disjoint delete/insert microbatches may execute concurrently."""
        ...

    @abstractmethod
    def physical_relation_generation(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> str | None:
        """Return a stable token for the current physical relation incarnation when available."""
        ...

    @abstractmethod
    def render_create_microbatch_state_table_sql(self, *, database: str | None, schema: str) -> str:
        """Render DDL that creates the direct microbatch state table when missing."""
        ...

    @abstractmethod
    def render_create_microbatch_state_index_sqls(
        self, *, database: str | None, schema: str
    ) -> tuple[str, ...]:
        """Render optional indexes for direct microbatch state."""
        ...

    @abstractmethod
    def describe_relation(self, *, connection: Any, relation: str) -> tuple[ColumnInfo, ...]:
        """Return relation column metadata."""
        ...

    @abstractmethod
    def probe_relation_read(self, *, connection: Any, relation: str) -> RelationReadProbe:
        """Probe a rendered relation; only the adapter's not-found errors become missing."""
        ...

    @abstractmethod
    def get_table_freshness_metadata(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> TableFreshnessMetadata:
        """Return comparable freshness metadata for one physical table."""
        ...

    @abstractmethod
    def get_tables_freshness_metadata(
        self,
        *,
        connection: Any,
        requests: tuple[TableFreshnessRequest, ...],
    ) -> dict[TableFreshnessRequest, TableFreshnessMetadata]:
        """Return one freshness outcome per requested table without failing the whole batch."""
        ...

    @abstractmethod
    def relation_usage(
        self,
        *,
        connection: Any,
        relations: tuple[RelationInfo, ...],
        window_seconds: int,
    ) -> RelationUsageReport:
        """Report reads of the relations by other queries in the last `window_seconds`."""
        ...

    @abstractmethod
    def query_column_names(self, *, connection: Any, sql: str) -> tuple[str, ...]:
        """Return column names produced by a SQL query."""
        ...

    @abstractmethod
    def render_max_cursor_at_or_before(
        self,
        *,
        relation: str,
        cursor_column: str,
        maximum_allowed: str,
        cursor_type: str | None,
        is_date: bool,
    ) -> str:
        """Render an eligible-cursor MAX query."""
        ...

    @abstractmethod
    def build_cursor_filter(
        self,
        *,
        cursor_column: str | None,
        start_cursor: CursorValue | None,
        end_cursor: CursorValue | None,
    ) -> str:
        """Build a WHERE clause fragment for cursor-bounded queries."""
        ...

    @abstractmethod
    def schema_exists(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str,
    ) -> bool:
        """Return whether the named schema exists in the warehouse."""
        ...

    @abstractmethod
    def render_create_schema(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        """Render SQL statements that create a schema if missing."""
        ...

    def render_drop_schema(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        """Optional: SQL that drops a schema and everything in it; unsupported by default."""

        del database
        raise AdapterUserError(
            f"the {self.adapter_name} adapter cannot drop schema '{schema}' with its contents",
            code="A714",
            help="sqb review needs a warehouse that can drop a schema it created",
        )

    @abstractmethod
    def render_create_table_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        """Render SQL statements that create or replace a table from a query."""
        ...

    @abstractmethod
    def render_create_view_as(self, *, destination: str, sql: str) -> tuple[str, ...]:
        """Render SQL statements that create or replace a view from a query."""
        ...

    @abstractmethod
    def render_table_function_call(self, *, target: str, call_suffix_sql: str) -> str:
        """Render a table function call for use as a FROM target."""
        ...

    @abstractmethod
    def render_udf_call(self, *, target: str, call_suffix_sql: str) -> str:
        """Render a scalar UDF call for use in an expression."""
        ...

    @abstractmethod
    def render_create_function(
        self,
        *,
        destination: str,
        arguments: tuple[Any, ...],
        returns: str,
        body_sql: str,
        return_columns: tuple[Any, ...] = (),
        language: FunctionLanguage = FunctionLanguage.SQL,
        runtime_version: str | None = None,
        entry_point: str | None = None,
        packages: tuple[str, ...] = (),
    ) -> tuple[str, ...]:
        """Render SQL statements that create or replace a SQL function."""
        ...

    @abstractmethod
    def render_append(
        self, *, destination: str, sql: str, columns: tuple[str, ...] | None = None
    ) -> tuple[str, ...]:
        """Render SQL statements that insert query rows into a destination."""
        ...

    @abstractmethod
    def render_delete_insert(
        self,
        *,
        destination: str,
        sql: str,
        unique_key: tuple[str, ...],
        columns: tuple[str, ...] | None = None,
    ) -> tuple[str, ...]:
        """Render SQL statements for delete-insert by unique key."""
        ...

    @abstractmethod
    def render_delete_insert_cursor(
        self,
        *,
        destination: str,
        sql: str,
        cursor_column: str,
        cursor_start: str,
        cursor_end: str,
        columns: tuple[str, ...] | None = None,
        cursor_type: str | None = None,
    ) -> tuple[str, ...]:
        """Render SQL statements for cursor-bounded delete-insert."""
        ...

    @abstractmethod
    def render_drop(self, *, destination: str, if_exists: bool = True) -> tuple[str, ...]:
        """Render SQL statements that drop a relation."""
        ...

    @abstractmethod
    def render_drop_view(self, *, destination: str, if_exists: bool = True) -> tuple[str, ...]:
        """Render SQL statements that drop a view relation."""
        ...

    @abstractmethod
    def drop_view(
        self,
        *,
        connection: Any,
        destination: str,
        if_exists: bool = True,
        statement_recorder: StatementRecorder,
    ) -> None:
        """Drop a view relation."""
        ...

    @abstractmethod
    def render_rename(self, *, origin: str, destination: str) -> tuple[str, ...]:
        """Render SQL statements that rename a relation."""
        ...

    @abstractmethod
    def render_rename_view(self, *, origin: str, destination: str) -> tuple[str, ...]:
        """Render SQL statements that rename a view relation."""
        ...

    @abstractmethod
    def render_swap(self, *, left: str, right: str) -> tuple[str, ...]:
        """Render SQL statements that swap two relations."""
        ...

    @abstractmethod
    def render_clone(
        self,
        *,
        origin: str,
        destination: str,
        hard_copy: bool = False,
        origin_is_transient: bool = False,
    ) -> tuple[str, ...]:
        """Render SQL statements that clone or copy a relation."""
        ...

    @abstractmethod
    def render_durable_clone(
        self, *, origin: str, destination: str, origin_is_transient: bool = False
    ) -> tuple[str, ...]:
        """Render SQL statements that clone/copy into a durable independent destination."""
        ...

    @abstractmethod
    def render_migration_stage(
        self,
        *,
        origin: str,
        stage: str,
        origin_is_transient: bool = False,
        stage_is_transient: bool | None = None,
    ) -> MigrationStagePlan:
        """Render statements that create a fresh, independent stage holding the origin's data."""
        ...

    @abstractmethod
    def capture_dependent_view_rebinds(
        self, *, connection: Any, database: str | None, schema: str, name: str
    ) -> tuple[str, ...]:
        """Capture statements that re-point identity-bound dependent views at this table name."""
        ...

    @abstractmethod
    def views_bind_to_relation_identity(self) -> bool:
        """Return whether views follow a renamed relation instead of re-resolving its name."""
        ...

    @abstractmethod
    def list_dependent_view_names(
        self, *, connection: Any, database: str | None, schema: str, name: str
    ) -> tuple[str, ...]:
        """Return qualified names of views bound to this relation."""
        ...

    @abstractmethod
    def render_create_old_name_view_state_table_sql(
        self, *, database: str | None, schema: str
    ) -> str:
        """Render DDL that creates the old-name view fact table when it is missing."""
        ...

    @abstractmethod
    def read_relation_grants(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str,
        name: str,
        relation_type: str,
    ) -> tuple[RelationGrant, ...]:
        """Return the privileges granted or denied on a relation and its columns."""
        ...

    @abstractmethod
    def render_relation_grants(
        self, *, grants: tuple[RelationGrant, ...], destination: str, columns: tuple[str, ...]
    ) -> tuple[str, ...]:
        """Render grants onto ``destination``, keeping column grants only for ``columns``."""
        ...

    @abstractmethod
    def render_replace_view_keeping_grants(
        self, *, destination: str, sql: str
    ) -> tuple[str, ...] | None:
        """Return statements that redefine an existing view keeping its privileges, if any."""
        ...

    @abstractmethod
    def read_view_definition(
        self, *, connection: Any, database: str | None, schema: str, name: str
    ) -> str | None:
        """Return the stored definition of a view, or None when unknown."""
        ...

    @abstractmethod
    def view_definition_matches(
        self, *, connection: Any, database: str | None, schema: str, name: str, sql: str
    ) -> bool:
        """Return whether the view's stored definition is ``sql`` as this warehouse stores it."""
        ...

    @abstractmethod
    def render_relation_revokes(
        self, *, grants: tuple[RelationGrant, ...], destination: str
    ) -> tuple[str, ...]:
        """Render statements that remove grants from ``destination``."""
        ...

    @abstractmethod
    def rename_view(
        self,
        *,
        connection: Any,
        origin: str,
        destination: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        """Rename a view in place, keeping its privileges."""
        ...

    @abstractmethod
    def supports_transactional_ddl(self) -> bool:
        """Return whether renames and state inserts can commit or roll back together."""
        ...

    @abstractmethod
    def render_seed_select_before_cursor(
        self,
        *,
        origin: str,
        cursor_column: str,
        cursor_end_exclusive: str,
        cursor_type: str | None,
    ) -> str:
        """Render a seed-select query that keeps rows before a cursor bound."""
        ...

    @abstractmethod
    def relation_names_match(self, *, left: str, right: str) -> bool:
        """Return whether two relation name strings identify the same adapter relation."""
        ...

    @abstractmethod
    def render_replace_table_from_relation(
        self, *, destination: str, origin: str
    ) -> tuple[str, ...]:
        """Render SQL statements that replace a destination table from a origin relation."""
        ...

    @abstractmethod
    def replace_table_from_relation(
        self,
        *,
        connection: Any,
        destination: str,
        origin: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        """Replace a destination table from a origin relation."""
        ...

    @abstractmethod
    def move_or_copy_relation(
        self,
        *,
        connection: Any,
        origin: str,
        destination: str,
        remove_origin: bool,
        allow_copy_fallback: bool,
        statement_recorder: StatementRecorder,
    ) -> None:
        """Move a relation to a destination, or copy it when explicit fallback is allowed."""
        ...

    @abstractmethod
    def render_add_columns(
        self, *, destination: str, columns: tuple[ColumnInfo, ...]
    ) -> tuple[str, ...]:
        """Render SQL statements that add columns to a table."""
        ...

    @abstractmethod
    def render_drop_columns(
        self, *, destination: str, column_names: tuple[str, ...]
    ) -> tuple[str, ...]:
        """Render SQL statements that drop columns from a table."""
        ...

    @abstractmethod
    def render_alter_column_types(
        self, *, destination: str, columns: tuple[ColumnInfo, ...]
    ) -> tuple[str, ...]:
        """Render SQL statements that alter column types on a table."""
        ...

    @abstractmethod
    def render_rename_column(
        self, *, destination: str, old_name: str, new_name: str
    ) -> tuple[str, ...]:
        """Render SQL that renames one table column in place, keeping its data."""
        ...

    @abstractmethod
    def render_merge(
        self,
        *,
        destination: str,
        sql: str,
        unique_key: tuple[str, ...],
        source_columns: tuple[str, ...] = (),
        exclude_columns: tuple[str, ...] = (),
    ) -> tuple[str, ...]:
        """Render SQL statements that merge query rows into a destination."""
        ...

    @abstractmethod
    def render_create_initial_snapshot_destination(
        self,
        *,
        table_type: str,
        destination: str,
        origin: str,
        snapshot_strategy: str | None,
        updated_at_column: str | None,
        observed_at_column: str | None,
        valid_from_column: str,
        valid_to_column: str,
        initial_valid_from: str | None,
    ) -> tuple[str, ...]:
        """Render SQL statements that create the initial snapshot target."""
        ...

    @abstractmethod
    def render_apply_timestamp_snapshot_changes(
        self,
        *,
        destination: str,
        origin: str,
        unique_key: tuple[str, ...],
        updated_at_column: str,
        observed_at_column: str | None,
        valid_from_column: str,
        valid_to_column: str,
        initial_valid_from: str | None,
        output_columns: tuple[str, ...],
        invalidate_hard_deletes: bool,
    ) -> tuple[str, ...]:
        """Render SQL statements that apply timestamp snapshot changes."""
        ...

    @abstractmethod
    def render_create_initial_historical_timestamp_snapshot_destination(
        self,
        *,
        table_type: str,
        destination: str,
        origin: str,
        unique_key: tuple[str, ...],
        updated_at_column: str,
        observed_at_column: str,
        valid_from_column: str,
        valid_to_column: str,
        output_columns: tuple[str, ...],
        invalidate_hard_deletes: bool,
    ) -> tuple[str, ...]:
        """Render SQL statements that create an initial historical timestamp snapshot."""
        ...

    @abstractmethod
    def render_create_initial_historical_timestamp_changes_destination(
        self,
        *,
        table_type: str,
        destination: str,
        origin: str,
        unique_key: tuple[str, ...],
        updated_at_column: str,
        valid_from_column: str,
        valid_to_column: str,
        output_columns: tuple[str, ...],
    ) -> tuple[str, ...]:
        """Render SQL statements that create an initial historical timestamp changes snapshot."""
        ...

    @abstractmethod
    def render_apply_historical_timestamp_snapshot_changes(
        self,
        *,
        destination: str,
        origin: str,
        unique_key: tuple[str, ...],
        updated_at_column: str,
        observed_at_column: str,
        valid_from_column: str,
        valid_to_column: str,
        output_columns: tuple[str, ...],
        invalidate_hard_deletes: bool,
    ) -> tuple[str, ...]:
        """Render SQL statements that apply historical timestamp snapshot changes."""
        ...

    @abstractmethod
    def render_apply_historical_timestamp_changes(
        self,
        *,
        destination: str,
        origin: str,
        unique_key: tuple[str, ...],
        updated_at_column: str,
        valid_from_column: str,
        valid_to_column: str,
        output_columns: tuple[str, ...],
    ) -> tuple[str, ...]:
        """Render SQL statements that apply historical timestamp change records."""
        ...

    @abstractmethod
    def render_apply_check_snapshot_changes(
        self,
        *,
        target: SnapshotChangeTarget,
        check_columns: tuple[str, ...],
        updated_at_column: str | None,
        observed_at_column: str | None,
        initial_valid_from: str | None,
        invalidate_hard_deletes: bool,
    ) -> tuple[str, ...]:
        """Render SQL statements that apply check snapshot changes."""
        ...

    @abstractmethod
    def render_create_initial_historical_check_snapshot_destination(
        self,
        *,
        table_type: str,
        destination: str,
        origin: str,
        unique_key: tuple[str, ...],
        check_columns: tuple[str, ...],
        observed_at_column: str,
        valid_from_column: str,
        valid_to_column: str,
        output_columns: tuple[str, ...],
        invalidate_hard_deletes: bool,
    ) -> tuple[str, ...]:
        """Render SQL statements that create an initial historical check snapshot."""
        ...

    @abstractmethod
    def render_apply_historical_check_snapshot_changes(
        self,
        *,
        destination: str,
        origin: str,
        unique_key: tuple[str, ...],
        check_columns: tuple[str, ...],
        observed_at_column: str,
        valid_from_column: str,
        valid_to_column: str,
        output_columns: tuple[str, ...],
        invalidate_hard_deletes: bool,
    ) -> tuple[str, ...]:
        """Render SQL statements that apply historical check snapshot changes."""
        ...

    @abstractmethod
    def render_current_timestamp(self) -> str:
        """Render the warehouse current timestamp expression."""
        ...

    @abstractmethod
    def validate_row_diff_keys(
        self,
        *,
        connection: Any,
        relation_sql: str,
        relation_label: str,
        keys: tuple[str, ...],
    ) -> None:
        """Validate row-diff unique keys for nulls and duplicates."""
        ...

    @abstractmethod
    def build_row_diff_equal_expression(
        self,
        *,
        column: str,
        column_info: ColumnInfo,
        tolerances: RowDiffTolerances | None,
    ) -> str:
        """Build a boolean equality expression for one row-diff column."""
        ...

    @abstractmethod
    def resolve_row_diff_tolerance(
        self,
        *,
        column: str,
        column_type: str,
        tolerances: RowDiffTolerances | None,
    ) -> RowDiffTolerance | None:
        """Resolve applicable row-diff tolerance for a column."""
        ...

    @abstractmethod
    def validate_row_diff_tolerance(self, *, column: str, tolerance: RowDiffTolerance) -> None:
        """Validate one row-diff tolerance definition."""
        ...

    @abstractmethod
    def normalize_row_diff_numeric_type(self, column_type: str) -> str | None:
        """Normalize one column type into a row-diff numeric family."""
        ...

    @abstractmethod
    def format_row_diff_decimal_sql(self, value: Decimal) -> str:
        """Format a decimal value for row-diff SQL."""
        ...

    @abstractmethod
    def default_schema(self) -> str | None:
        """Return the adapter's default schema name, or None if schema is required."""
        ...

    @abstractmethod
    def default_database(self) -> str | None:
        """Return the adapter's default database name, or None if database is required."""
        ...

    @abstractmethod
    def star_exclude_keyword(self) -> str:
        """Return the SQL keyword for SELECT * EXCLUDE/EXCEPT syntax."""
        ...

    @abstractmethod
    def render_qualified_name(
        self,
        *,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> str | None:
        """Render a fully qualified relation name for this adapter."""
        ...

    @abstractmethod
    def render_identifier(self, name: str) -> str:
        """Render one SQL identifier for this adapter."""
        ...

    @abstractmethod
    def render_exact_identifier(self, name: str) -> str:
        """Render one explicitly quoted identifier without changing its logical spelling."""
        ...

    @abstractmethod
    def render_framework_type(self, type_name: FrameworkType) -> str:
        """Render one framework-internal logical type for this adapter."""
        ...

    @abstractmethod
    def render_loader_logical_type(self, type_name: LoaderLogicalType) -> str:
        """Render one source-loader logical type for this adapter."""
        ...

    @abstractmethod
    def render_typed_scalar(self, *, value: SqlValue) -> str:
        """Render one validated typed SQL scalar."""
        ...

    @abstractmethod
    def render_typed_value_list(self, *, value: SqlValue) -> str:
        """Render one validated collection as a parenthesized SQL value list."""
        ...

    @abstractmethod
    def render_typed_array(self, *, value: SqlValue) -> str:
        """Render one validated collection as an adapter-native array."""
        ...

    @abstractmethod
    def render_typed_object(self, *, value: SqlValue) -> str:
        """Render one validated object as an adapter-native JSON expression."""
        ...

    @abstractmethod
    def render_loader_value_literal(
        self, *, value: object, logical_type: LoaderLogicalType | None
    ) -> str:
        """Render one source-loader row value as a SQL literal/expression."""
        ...

    @abstractmethod
    def render_loader_rows_select(
        self,
        *,
        rows: tuple[dict[str, object], ...],
        column_names: tuple[str, ...],
        column_sql_types: dict[str, str],
        inferred_types: dict[str, LoaderLogicalType],
    ) -> str:
        """Render source-loader rows as a SELECT statement for staging writes."""
        ...

    @abstractmethod
    def render_source_expression_cast(
        self, *, expression: str, target_type: str, alias: str
    ) -> str:
        """Render a cast projection for source expression type enforcement."""
        ...

    @abstractmethod
    def render_source_expression_relation(self, *, expression: str) -> str:
        """Render a source expression as a SQL table factor."""
        ...

    @abstractmethod
    def render_source_freshness_max_query(
        self, *, column: str, source_relation: str, source_is_subquery: bool, where_sql: str
    ) -> str:
        """Render a max-column source freshness query over a source table factor."""
        ...

    @abstractmethod
    def render_source_expression_cast_subquery(
        self, *, source_relation: str, projections: tuple[str, ...]
    ) -> str:
        """Render a type-enforced source expression as a SQL table factor."""
        ...

    @abstractmethod
    def render_source_relation_cast_subquery(
        self,
        *,
        source_relation: str,
        cast_projections: tuple[str, ...],
        cast_column_names: tuple[str, ...],
        all_columns_cast: bool,
    ) -> str:
        """Render a type-enforced source relation as a SQL table factor."""
        ...

    @abstractmethod
    def requires_derived_table_aliases(self) -> bool:
        """Return whether derived table factors need explicit aliases."""
        ...

    @abstractmethod
    def render_set_difference_operator(self) -> str:
        """Render the set-difference operator keyword for this adapter."""
        ...

    @abstractmethod
    def render_create_fingerprint_table_sql(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> str:
        """Render DDL that creates the fingerprint table when missing."""
        ...

    @abstractmethod
    def render_read_latest_fingerprints_sql(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> str:
        """Render SQL that reads latest fingerprint rows per identity."""
        ...

    @abstractmethod
    def render_read_latest_source_freshness_sql(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> str:
        """Render SQL that reads latest source freshness rows per identity."""
        ...

    @abstractmethod
    def render_create_fingerprint_index_sqls(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        """Render optional fingerprint table index DDL statements."""
        ...

    @abstractmethod
    def render_create_source_freshness_table_sql(self, *, database: str | None, schema: str) -> str:
        """Render DDL that creates the source freshness table when it is missing."""
        ...

    @abstractmethod
    def render_create_source_freshness_index_sqls(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        """Render optional source freshness table index DDL statements."""
        ...

    @abstractmethod
    def render_create_consumption_table_sql(self, *, database: str | None, schema: str) -> str:
        """Render DDL that creates the consumption table when it is missing."""
        ...

    @abstractmethod
    def render_create_consumption_index_sqls(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        """Render optional consumption table index DDL statements."""
        ...

    @abstractmethod
    def render_current_utc_timestamp(self) -> str:
        """Render the warehouse clock as a UTC timestamp expression."""
        ...

    @abstractmethod
    def render_insert_source_freshness_records_sql(
        self,
        *,
        database: str | None,
        schema: str,
        records: tuple[SourceFreshnessRecord, ...],
    ) -> str:
        """Render DML that appends source freshness records."""
        ...

    @abstractmethod
    def render_create_node_result_table_sql(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> str:
        """Render DDL that creates the node result table when it is missing."""
        ...

    @abstractmethod
    def render_create_node_result_index_sqls(
        self,
        *,
        database: str | None,
        schema: str,
    ) -> tuple[str, ...]:
        """Render optional node result table index DDL statements."""
        ...

    @abstractmethod
    def render_create_audit_result_table_sql(self, *, database: str | None, schema: str) -> str:
        """Render DDL that creates the audit result table when it is missing."""
        ...

    @abstractmethod
    def render_create_audit_result_index_sqls(
        self, *, database: str | None, schema: str
    ) -> tuple[str, ...]:
        """Render optional audit result table index DDL statements."""
        ...

    @abstractmethod
    def render_create_janitor_event_table_sql(self, *, database: str | None, schema: str) -> str:
        """Render DDL that creates the janitor audit event table when it is missing."""
        ...

    @abstractmethod
    def render_create_migration_state_table_sql(self, *, database: str | None, schema: str) -> str:
        """Render DDL that creates the model migration event table when it is missing."""
        ...

    @abstractmethod
    def render_create_column_migration_state_table_sql(
        self, *, database: str | None, schema: str
    ) -> str:
        """Render DDL that creates the column migration event table when it is missing."""
        ...

    @abstractmethod
    def render_prune_fingerprint_history_sql(
        self,
        *,
        database: str | None,
        schema: str,
        retain_versions: int,
    ) -> str:
        """Render SQL that prunes old fingerprint history rows."""
        ...

    @abstractmethod
    def render_prune_source_freshness_history_sql(
        self,
        *,
        database: str | None,
        schema: str,
        retain_versions: int,
    ) -> str:
        """Render SQL that prunes old source freshness history rows."""
        ...

    @abstractmethod
    def render_prune_consumption_history_sql(
        self,
        *,
        database: str | None,
        schema: str,
        retain_versions: int,
    ) -> str:
        """Render SQL that prunes old consumption executions."""
        ...

    @abstractmethod
    def sql_analysis_dialect(self) -> str | None:
        """Return the SQL analysis dialect name for this adapter, if any."""
        ...

    @abstractmethod
    def expression_inference_profile(self) -> ExpressionInferenceProfile:
        """Return static SQL expression inference behavior for this adapter."""
        ...

    @abstractmethod
    def render_cursor_bound_literal(self, *, value: str, cursor_type: str | None) -> str:
        """Render one cursor bound literal for this adapter and cursor type."""
        ...

    @abstractmethod
    def default_table_promotion_mode(self) -> TablePromotionMode:
        """Return the adapter default table promotion mode."""
        ...

    @abstractmethod
    def default_promotion_strategy(self) -> PromotionStrategy:
        """Return the adapter default staged table promotion strategy."""
        ...
