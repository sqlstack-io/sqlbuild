from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import Any, ClassVar

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    MaterializationType,
    ModelPlanEntry,
    PlanAction,
    PlanReason,
    ScenarioArtifactKind,
    ScenarioExecutionPlan,
    ScenarioFixturePlan,
    ScenarioGraphPlan,
    ScenarioRelationMap,
    ScenarioRelationPlan,
    SeedCsvSettings,
    SeedPlanEntry,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import ColumnInfo, QueryResult, RelationInfo
from sqlbuild.spec.contracts.constants import DEFAULT_SEED_CSV_SETTINGS


class ScenarioFixtureTestAdapter(BaseAdapter):
    """Adapter that records scenario fixture materialization calls."""

    adapter_name: ClassVar[str] = "scenario-fixture-test"

    def __init__(self, *, fail_on_target: str | None = None) -> None:
        self.fail_on_target: str | None = fail_on_target
        self.executed_sql: list[str] = []

    def connect(self, config: dict[str, object]) -> object:
        del config
        return object()

    def _execute(self, connection: object, sql: str) -> object:
        del connection
        self.executed_sql.append(sql)
        _SCENARIO_EXECUTE_STRATEGIES[
            self.fail_on_target is not None and self.fail_on_target in sql
        ](target=self.fail_on_target)
        return object()

    def close(self, connection: object) -> None:
        del connection


class ScenarioCatalogTestAdapter(ScenarioFixtureTestAdapter):
    """Adapter whose catalog lists only the named scenario relations."""

    adapter_name: ClassVar[str] = "scenario-catalog-test"

    def __init__(
        self,
        *,
        relation_types: dict[str, str],
        fail_listing: bool = False,
        interrupt_on_create: bool = False,
        authoritative: bool = True,
        catalog_schema: str = "scenario_schema",
        prefix_listing: bool = False,
    ) -> None:
        super().__init__()
        self.prefix_listing: bool = prefix_listing
        self.prefix_listings: list[str] = []
        self.relation_types: dict[str, str] = relation_types
        self.fail_listing: bool = fail_listing
        self.authoritative: bool = authoritative
        self.catalog_schema: str = catalog_schema
        self.interrupt_on_create: bool = interrupt_on_create
        self.listing_count: int = 0

    def lists_relations_authoritatively(self, *, database: str | None, schema: str) -> bool:
        del database, schema
        return self.authoritative

    def list_relations(
        self,
        *,
        connection: Any,
        database: str | None,
        schemas: tuple[str, ...] | None,
        names: tuple[str, ...] | None = None,
    ) -> tuple[RelationInfo, ...]:
        del connection
        self.listing_count += 1
        _CATALOG_LISTING_STRATEGIES[self.fail_listing]()
        in_scope: bool = self.authoritative and self.catalog_schema in (schemas or ())
        listed_names: set[str] = set(self.relation_types).intersection(
            {True: names or (), False: ()}[in_scope]
        )
        return tuple(
            RelationInfo(
                database=database,
                schema=self.catalog_schema,
                name=name,
                relation_type=self.relation_types[name],
            )
            for name in sorted(listed_names)
        )

    def list_relations_with_prefix(
        self, *, connection: Any, database: str | None, schemas: tuple[str, ...], prefix: str
    ) -> tuple[RelationInfo, ...] | None:
        del connection
        self.prefix_listings.extend((prefix,) * self.prefix_listing)
        listed: tuple[RelationInfo, ...] = tuple(
            RelationInfo(
                database=database,
                schema=schemas[0],
                name=name.lower(),
                relation_type=relation_type,
            )
            for name, relation_type in filter(
                lambda item: item[0].lower().startswith(prefix.lower()),
                sorted(self.relation_types.items()),
            )
        )
        return (None, listed)[self.prefix_listing]

    def _execute(self, connection: object, sql: str) -> object:
        _CATALOG_CREATE_STRATEGIES[self.interrupt_on_create and sql.startswith("CREATE ")]()
        return super()._execute(connection, sql)


class ScenarioSnapshotCaptureStepsTestAdapter(BaseAdapter):
    """Adapter that records capture orchestration operations."""

    adapter_name: ClassVar[str] = "scenario-snapshot-steps-test"

    def __init__(
        self,
        *,
        fail_on_create_target: str | None = None,
        fail_on_seed: bool = False,
        fail_on_query_target: str | None = None,
    ) -> None:
        self.fail_on_create_target: str | None = fail_on_create_target
        self.fail_on_seed: bool = fail_on_seed
        self.fail_on_query_target: str | None = fail_on_query_target
        self.events: list[str] = []

    def connect(self, config: dict[str, object]) -> object:
        del config
        return object()

    def _execute(self, connection: object, sql: str) -> object:
        del connection
        self.events.append(sql)
        return object()

    def close(self, connection: object) -> None:
        del connection

    def create_table_as(
        self,
        connection: Any,
        *,
        destination: str,
        sql: str,
        config: dict[str, Any] | None = None,
        statement_recorder: StatementRecorder,
    ) -> None:
        del connection, sql, config
        self.events.append(f"create:{destination}")
        _CAPTURE_CREATE_STRATEGIES[
            self.fail_on_create_target is not None and self.fail_on_create_target in destination
        ]()
        statement_recorder.record(f"CREATE TABLE {destination}")

    def drop(
        self,
        connection: Any,
        *,
        destination: str,
        if_exists: bool = True,
        statement_recorder: StatementRecorder,
    ) -> None:
        del connection, if_exists
        self.events.append(f"drop:{destination}")
        statement_recorder.record(f"DROP TABLE {destination}")

    def load_seed(
        self,
        connection: Any,
        *,
        destination: str,
        file_path: Path,
        columns: tuple[ColumnInfo, ...],
        csv_settings: SeedCsvSettings = DEFAULT_SEED_CSV_SETTINGS,
        replace: bool = True,
        infer_types: bool = False,
        statement_recorder: StatementRecorder,
    ) -> None:
        del connection, file_path, columns, csv_settings, replace, infer_types
        self.events.append(f"seed:{destination}")
        _CAPTURE_SEED_STRATEGIES[self.fail_on_seed]()
        statement_recorder.record(f"LOAD SEED {destination}")

    def query(self, connection: Any, sql: str, *, limit: int | None) -> QueryResult:
        del connection, limit
        self.events.append(f"query:{sql}")
        _CAPTURE_QUERY_FAILURE_STRATEGIES[
            self.fail_on_query_target is not None and self.fail_on_query_target in sql
        ]()
        query_key: tuple[bool, bool, bool, bool] = (
            "COUNT(*)" in sql,
            "__sqb_51b385aebe20__source__raw__orders" in sql,
            "__sqb_51b385aebe20__ref__stg_customers" in sql,
            "__sqb_51b385aebe20__seed__country_codes" in sql,
        )
        return _CAPTURE_QUERY_RESULTS.get(query_key, _unexpected_query)(sql)

    def describe_relation(self, connection: Any, relation: str) -> tuple[ColumnInfo, ...]:
        del connection
        relation_key: tuple[bool, bool, bool] = (
            "__sqb_51b385aebe20__source__raw__orders" in relation,
            "__sqb_51b385aebe20__ref__stg_customers" in relation,
            "__sqb_51b385aebe20__seed__country_codes" in relation,
        )
        return _CAPTURE_RELATION_COLUMNS.get(relation_key, _unexpected_describe)(relation)


def build_scenario_fixture_plan(
    *,
    kind: ScenarioArtifactKind = ScenarioArtifactKind.SOURCE,
    logical_name: str = "raw__orders",
    target_name: str = "__sqb_51b385aebe20__source__raw__orders",
    sql: str = "WITH helper_orders AS (SELECT 1 AS order_id) SELECT * FROM helper_orders",
) -> ScenarioFixturePlan:
    return ScenarioFixturePlan(
        kind=kind,
        logical_name=logical_name,
        destination=CompiledRelationLocation(
            database=None,
            schema="scenario_schema",
            name=target_name,
            qualified_name=f"scenario_schema.{target_name}",
        ),
        sql=sql,
    )


def executed_create_table_sql(adapter: ScenarioFixtureTestAdapter) -> tuple[str, ...]:
    statements: dict[bool, list[str]] = {True: [], False: []}
    for sql in adapter.executed_sql:
        statements["CREATE OR REPLACE TABLE" in sql].append(sql)
    return tuple(statements[True])


def executed_drop_sql(adapter: ScenarioFixtureTestAdapter) -> tuple[str, ...]:
    statements: dict[bool, list[str]] = {True: [], False: []}
    for sql in adapter.executed_sql:
        statements[sql.startswith("DROP ")].append(sql)
    return tuple(statements[True])


def build_scenario_cleanup_test_plan(
    *,
    model_materialization_type: MaterializationType = MaterializationType.TABLE,
    model_target_name: str = "__sqb_51b385aebe20__model__daily_revenue",
) -> ScenarioExecutionPlan:
    source_fixture: ScenarioFixturePlan = build_scenario_fixture_plan()
    ref_fixture: ScenarioFixturePlan = build_scenario_fixture_plan(
        kind=ScenarioArtifactKind.REF,
        logical_name="stg_customers",
        target_name="__sqb_51b385aebe20__ref__stg_customers",
        sql="SELECT 10 AS customer_id",
    )
    seed_fixture: ScenarioFixturePlan = build_scenario_fixture_plan(
        kind=ScenarioArtifactKind.SEED,
        logical_name="country_codes",
        target_name="__sqb_51b385aebe20__seed__country_codes",
        sql="SELECT 'US' AS country_code",
    )
    model_target: CompiledRelationLocation = CompiledRelationLocation(
        database=None,
        schema="scenario_schema",
        name=model_target_name,
        qualified_name=f"scenario_schema.{model_target_name}",
    )
    return ScenarioExecutionPlan(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.SQL_SCENARIO,
            name="revenue__customer_refund",
        ),
        name="revenue__customer_refund",
        graph_plan=ScenarioGraphPlan(
            key=CompiledObjectKey(
                resource_type=CompiledResourceType.SQL_SCENARIO,
                name="revenue__customer_refund",
            ),
            name="revenue__customer_refund",
            model_names=("daily_revenue",),
        ),
        relation_plan=ScenarioRelationPlan(
            scenario_name="revenue__customer_refund",
            relation_map=ScenarioRelationMap(
                scenario_name="revenue__customer_refund",
                hash_prefix="51b385aebe20",
            ),
            model_locations={
                "daily_revenue": model_target,
                "stg_customers": ref_fixture.destination,
            },
            source_fixture_locations={"raw__orders": source_fixture.destination},
            ref_fixture_locations={"stg_customers": ref_fixture.destination},
            seed_fixture_locations={"country_codes": seed_fixture.destination},
        ),
        fixture_plans=(source_fixture, ref_fixture, seed_fixture),
        model_entries=(
            ModelPlanEntry(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.MODEL,
                    name="daily_revenue",
                ),
                name="daily_revenue",
                relative_path=Path("sql/models/daily_revenue.sql"),
                materialization_type=model_materialization_type,
                action=PlanAction.CREATE_TABLE,
                reason=PlanReason.FIRST_RUN,
                destination=model_target,
                fingerprint_query_sql="SELECT 1 AS revenue",
                resolved_sql="SELECT 1 AS revenue",
                logical_ddl="CREATE TABLE daily_revenue AS SELECT 1 AS revenue",
            ),
        ),
    )


def build_scenario_cleanup_test_plan_with_project_seed() -> ScenarioExecutionPlan:
    plan: ScenarioExecutionPlan = build_scenario_cleanup_test_plan()
    seed_target: CompiledRelationLocation = CompiledRelationLocation(
        database=None,
        schema="scenario_schema",
        name="__sqb_51b385aebe20__seed__country_codes",
        qualified_name="scenario_schema.__sqb_51b385aebe20__seed__country_codes",
    )
    return ScenarioExecutionPlan(
        key=plan.key,
        name=plan.name,
        graph_plan=plan.graph_plan,
        relation_plan=plan.relation_plan,
        fixture_plans=plan.fixture_plans[:2],
        seed_entries=(
            SeedPlanEntry(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.SEED,
                    name="country_codes",
                ),
                name="country_codes",
                destination=seed_target,
                file_path=Path("sql/seeds/country_codes.csv"),
                columns=(),
                csv_settings=DEFAULT_SEED_CSV_SETTINGS,
            ),
        ),
    )


def build_scenario_model_test_plan(
    *, model_entries: tuple[ModelPlanEntry, ...]
) -> ScenarioExecutionPlan:
    source_target: CompiledRelationLocation = CompiledRelationLocation(
        database=None,
        schema="scenario_schema",
        name="__sqb_51b385aebe20__source__raw__orders",
        qualified_name="scenario_schema.__sqb_51b385aebe20__source__raw__orders",
    )
    seed_target: CompiledRelationLocation = CompiledRelationLocation(
        database=None,
        schema="scenario_schema",
        name="__sqb_51b385aebe20__seed__country_codes",
        qualified_name="scenario_schema.__sqb_51b385aebe20__seed__country_codes",
    )
    model_locations: dict[str, CompiledRelationLocation] = {
        entry.name: entry.destination for entry in model_entries
    }
    return ScenarioExecutionPlan(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.SQL_SCENARIO,
            name="revenue__customer_refund",
        ),
        name="revenue__customer_refund",
        graph_plan=ScenarioGraphPlan(
            key=CompiledObjectKey(
                resource_type=CompiledResourceType.SQL_SCENARIO,
                name="revenue__customer_refund",
            ),
            name="revenue__customer_refund",
            model_names=tuple(entry.name for entry in model_entries),
            source_fixture_names=("raw__orders",),
            seed_names=("country_codes",),
        ),
        relation_plan=ScenarioRelationPlan(
            scenario_name="revenue__customer_refund",
            relation_map=ScenarioRelationMap(
                scenario_name="revenue__customer_refund",
                hash_prefix="51b385aebe20",
            ),
            model_locations=model_locations,
            seed_locations={"country_codes": seed_target},
            source_fixture_locations={"raw__orders": source_target},
            source_map={},
        ),
        model_entries=model_entries,
    )


def build_scenario_model_entry(
    *,
    name: str = "daily_revenue",
    target_name: str = "__sqb_51b385aebe20__model__daily_revenue",
    resolved_sql: str = "SELECT * FROM scenario_schema.__sqb_51b385aebe20__source__raw__orders",
    materialization_type: MaterializationType = MaterializationType.TABLE,
    action: PlanAction = PlanAction.CREATE_TABLE,
) -> ModelPlanEntry:
    return ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name),
        name=name,
        relative_path=Path(f"sql/models/{name}.sql"),
        materialization_type=materialization_type,
        action=action,
        reason=PlanReason.FIRST_RUN,
        destination=CompiledRelationLocation(
            database=None,
            schema="scenario_schema",
            name=target_name,
            qualified_name=f"scenario_schema.{target_name}",
        ),
        fingerprint_query_sql=resolved_sql,
        resolved_sql=resolved_sql,
        logical_ddl="",
    )


def _no_op_strategy(*args: object, **kwargs: object) -> None:
    del args, kwargs


def _raise_scenario_execute_error(*, target: str | None) -> None:
    raise RuntimeError(f"failed target {target}")


def _raise_capture_create_error() -> None:
    raise RuntimeError("fixture create failed")


def _raise_capture_seed_error() -> None:
    raise RuntimeError("seed load failed")


def _raise_capture_query_error() -> None:
    raise RuntimeError("warehouse read failed")


def _constant_query_result(result: QueryResult) -> Callable[[str], QueryResult]:
    def resolve(_sql: str) -> QueryResult:
        return result

    return resolve


def _unexpected_query(sql: str) -> QueryResult:
    raise RuntimeError(f"unexpected query: {sql}")


def _constant_columns(
    columns: tuple[ColumnInfo, ...],
) -> Callable[[str], tuple[ColumnInfo, ...]]:
    def resolve(_relation: str) -> tuple[ColumnInfo, ...]:
        return columns

    return resolve


def _unexpected_describe(relation: str) -> tuple[ColumnInfo, ...]:
    raise RuntimeError(f"unexpected describe: {relation}")


_SCENARIO_EXECUTE_STRATEGIES: dict[bool, Callable[..., None]] = {
    True: _raise_scenario_execute_error,
    False: _no_op_strategy,
}
_CAPTURE_CREATE_STRATEGIES: dict[bool, Callable[..., None]] = {
    True: _raise_capture_create_error,
    False: _no_op_strategy,
}
_CAPTURE_SEED_STRATEGIES: dict[bool, Callable[..., None]] = {
    True: _raise_capture_seed_error,
    False: _no_op_strategy,
}
_CAPTURE_QUERY_FAILURE_STRATEGIES: dict[bool, Callable[..., None]] = {
    True: _raise_capture_query_error,
    False: _no_op_strategy,
}
_CAPTURE_QUERY_RESULTS: dict[tuple[bool, bool, bool, bool], Callable[[str], QueryResult]] = {
    (True, True, False, False): _constant_query_result(
        QueryResult(columns=("count",), rows=((1,),))
    ),
    (True, False, True, False): _constant_query_result(
        QueryResult(columns=("count",), rows=((1,),))
    ),
    (True, False, False, True): _constant_query_result(
        QueryResult(columns=("count",), rows=((1,),))
    ),
    (False, True, False, False): _constant_query_result(
        QueryResult(columns=("order_id",), rows=((1,),))
    ),
    (False, False, True, False): _constant_query_result(
        QueryResult(columns=("customer_id",), rows=((10,),))
    ),
    (False, False, False, True): _constant_query_result(
        QueryResult(columns=("country_code",), rows=(("US",),))
    ),
}
_CAPTURE_RELATION_COLUMNS: dict[
    tuple[bool, bool, bool], Callable[[str], tuple[ColumnInfo, ...]]
] = {
    (True, False, False): _constant_columns((ColumnInfo(name="order_id", type="INTEGER"),)),
    (False, True, False): _constant_columns((ColumnInfo(name="customer_id", type="INTEGER"),)),
    (False, False, True): _constant_columns((ColumnInfo(name="country_code", type="VARCHAR"),)),
}


def _raise_catalog_unavailable() -> None:
    raise RuntimeError("catalog unavailable")


def _raise_interrupt() -> None:
    raise KeyboardInterrupt


_CATALOG_LISTING_STRATEGIES: dict[bool, Callable[[], None]] = {
    True: _raise_catalog_unavailable,
    False: lambda: None,
}
_CATALOG_CREATE_STRATEGIES: dict[bool, Callable[[], None]] = {
    True: _raise_interrupt,
    False: lambda: None,
}


class ScenarioInterruptTestAdapter(ScenarioFixtureTestAdapter):
    """Adapter that records statement cancellation and connection closes."""

    adapter_name: ClassVar[str] = "scenario-interrupt-test"

    def __init__(self, *, cancellable: bool) -> None:
        super().__init__()
        self.cancellable: bool = cancellable
        self.events: list[str] = []

    def interrupt_connection(self, connection: Any) -> bool:
        self.events.append(f"interrupt:{connection}")
        return self.cancellable

    def close(self, connection: object) -> None:
        self.events.append(f"close:{connection}")
