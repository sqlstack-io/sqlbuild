from __future__ import annotations

import functools
import itertools
import json
import random
from collections.abc import Callable
from contextlib import suppress
from dataclasses import dataclass, field, replace
from itertools import chain
from operator import attrgetter
from pathlib import Path
from typing import TYPE_CHECKING, Any, Literal, cast

import pytest

import sqlbuild._native as native_module
from sqlbuild._native import (
    CollectionRendering,
    ColumnLineageMode,
    CompactLineageFacts,
    CompileAdapterContext,
    DynamicColumnContractProof,
    InferredColumn,
    InferredNullability,
    ModelSqlAnalysis,
    NativePivotTables,
    SchemaDynamicColumnFamily,
    build_compile_inputs,
    cursor_intrinsics_analysis_sql,
    discover_project_inputs,
    infer_native_expression_source_shapes,
    model_placeholders,
    prove_native_dynamic_contracts,
    session_outcome_analysis,
    substitute_placeholder_defaults,
)
from sqlbuild._native import dynamic_contract_proof as contract_proof
from sqlbuild.adapter.contract.models import ExpressionInferenceProfile
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.cli.entry.main.entry import main
from sqlbuild.compiler.compile.exceptions import CompileInputError
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.analysis_session._test_types import (
    AdapterRuleCallbackTestCase,
)
from tests.integration.src.sqlbuild.compiler.helpers import GoldenEntry, golden_entry

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView, ModelInput

LINEAGE_NATIVE: str = "native"

_PROJECT_TOML: str = (
    'name = "orders_analysis"\nadapter = "duckdb"\n\n[connection]\ndatabase = "orders.duckdb"\n'
)
_SOURCES: str = """sources:
  - name: raw_orders
    description: Orders feed.
    expression: >-
      (SELECT 1 AS order_id, 10 AS customer_id, CAST(5 AS DOUBLE) AS amount, 'placed' AS status)
    columns:
      - name: order_id
        type: INTEGER
      - name: customer_id
        type: INTEGER
      - name: amount
        type: DOUBLE
      - name: status
        type: VARCHAR
  - name: raw_customers
    description: Customers feed.
    expression: >-
      (SELECT 10 AS customer_id, 'Ada' AS Customer_Name, 'east' AS region)
  - name: raw_payments
    description: Typed payments feed.
    expression: >-
      (SELECT 10 AS customer_id, 'east' AS region, CAST(5 AS DECIMAL(10, 2)) AS amount)
    columns:
      - name: customer_id
        type: INTEGER
      - name: region
        type: VARCHAR
      - name: amount
        type: DECIMAL(10, 2)
  - name: raw_events
    description: Untyped events feed.
    columns:
      - name: event_id
      - name: order_id
      - name: kind
"""
_INITIAL_RELATIONS: tuple[tuple[str, tuple[str, ...]], ...] = (
    ('__source("raw_orders")', ("order_id", "customer_id", "amount", "status")),
    ('__source("raw_customers")', ("customer_id", "Customer_Name", "region")),
    ('__source("raw_events")', ("event_id", "order_id", "kind")),
)
_DUCKDB_PROFILE: ExpressionInferenceProfile = ExpressionInferenceProfile(
    sql_analysis_dialect="duckdb"
)
_PIVOT_HEADER: str = """MODEL (
  description "Order amounts pivoted by status",
  contract enforced,
  materialized table,
  columns (customer_id (type INTEGER)),
  dynamic_columns (
    status_amounts (
      pivot_column {pivot},
      value_column {value},
      aggregate {aggregate},
      type DOUBLE
    )
  ),
);

"""
_PIVOT_BODIES: tuple[str, ...] = (
    'PIVOT __source("raw_orders")\nON status\nUSING {aggregate}(amount)\nGROUP BY customer_id',
    'PIVOT __source("raw_orders")\nON status\nUSING {aggregate}(CAST(amount AS DECIMAL(12, 2)))',
    "WITH base AS (\n  SELECT customer_id, status, CAST(amount AS DOUBLE) AS amount\n"
    '  FROM __source("raw_orders")\n)\nSELECT * FROM base\n'
    "PIVOT ({aggregate}(amount) FOR status IN (ANY ORDER BY status))",
    'WITH base AS (\n  SELECT * FROM __source("raw_orders")\n),\npivoted AS (\n'
    "  PIVOT base ON status USING {aggregate}(amount) GROUP BY customer_id\n)\n"
    "SELECT * FROM pivoted",
    "PIVOT __source(\"raw_orders\")\nON status IN ('placed', 'shipped')\n"
    "USING {aggregate}(amount)\nGROUP BY customer_id",
    'PIVOT __source("raw_events")\nON kind\nUSING {aggregate}(order_id)\nGROUP BY event_id',
    'SELECT * FROM __source("raw_orders") o\nJOIN __source("raw_customers") c USING (customer_id)',
)
_PIVOT_AGGREGATES: tuple[str, ...] = ("MAX", "MIN", "ANY_VALUE", "SUM")
_TYPED_PIVOT: str = (
    _PIVOT_HEADER.format(pivot="status", value="amount", aggregate="MAX")
    + _PIVOT_BODIES[0].format(aggregate="MAX")
    + "\n"
)
_TYPED_PIVOT_PASSTHROUGH: str = (
    _PIVOT_HEADER.format(pivot="status", value="amount", aggregate="MAX")
    + 'SELECT * FROM __ref("status_amounts")\n'
)
_ADAPTER_RULE_SQL: dict[str, tuple[str, str]] = {
    "adapter_rules_cte": (
        "  contract enforced,\n",
        "WITH base AS (\n  SELECT customer_id, IFF(region IS NULL, 'a', NULL) AS maybe_label,\n"
        "    IFF(region IS NULL, 'a', 'b') AS label, UPPER(region) AS upper_region,\n"
        "    LOWER('X') AS lower_constant, SPLIT_PART(region, '_', 1) AS region_prefix,\n"
        "    REPLACE(region, '_', '-') AS dashed_region\n"
        '  FROM __source("raw_payments")\n)\n'
        "SELECT customer_id, maybe_label, label, upper_region, lower_constant, region_prefix,\n"
        "  dashed_region\nFROM base",
    ),
    "adapter_rules_untyped_cte": (
        "  contract enforced,\n",
        'WITH base AS (SELECT event_id, kind FROM __source("raw_events"))\n'
        "SELECT event_id, IFF(kind IS NULL, 'a', NULL) AS maybe_label, UPPER(kind) AS upper_kind,\n"
        "  LOWER('X') AS lower_constant, SPLIT_PART(kind, '_', 1) AS kind_prefix\nFROM base",
    ),
    "adapter_rules_staging": (
        "",
        "SELECT event_id, IFF(kind IS NULL, 'a', NULL) AS maybe_label, UPPER(kind) AS upper_kind,\n"
        "  LISTAGG(kind) AS kinds, TO_DATE('2026-01-01') AS start_date\n"
        'FROM __source("raw_events")\nGROUP BY event_id, kind',
    ),
    "long_s_cte": (
        "  contract enforced,\n",
        "WITH base AS (SELECT event_id, 'ſplit_part' AS label FROM __source(\"raw_events\"))\n"
        "SELECT event_id, label FROM base",
    ),
    "long_s_staging": (
        "",
        "SELECT event_id, 'ſplit_part' AS label, kind FROM __source(\"raw_events\")",
    ),
    "dotless_i_staging": (
        "",
        "SELECT event_id, 'lıstagg' AS label FROM __source(\"raw_events\")",
    ),
    "umlaut_cte": (
        "  contract enforced,\n",
        "WITH base AS (SELECT event_id, 'grüße' AS label FROM __source(\"raw_events\"))\n"
        "SELECT event_id, label FROM base",
    ),
}
ADAPTER_RULE_MODELS: dict[str, str] = {
    f"models/adapter_rules/{name}.sql": (
        f'MODEL (\n  description "Adapter rule model {name}",\n{header});\n\n{sql}\n'
    )
    for name, (header, sql) in _ADAPTER_RULE_SQL.items()
}
_CONTRACT_CTE_MODELS: dict[str, str] = {
    "models/marts/payments_by_customer.sql": (
        'MODEL (\n  description "Payments passed through a CTE",\n  contract enforced,\n'
        "  columns (customer_id (type INTEGER), region (type VARCHAR)),\n);\n\n"
        'WITH base AS (SELECT customer_id, region FROM __source("raw_payments"))\n'
        "SELECT customer_id, region FROM base\n"
    ),
    "models/marts/payment_totals.sql": (
        'MODEL (\n  description "Payment totals aggregated in a CTE",\n  contract enforced,\n'
        "  columns (total_amount (type DECIMAL(38, 2)), region (type VARCHAR)),\n);\n\n"
        "WITH base AS (\n  SELECT SUM(amount) AS total_amount, region\n"
        '  FROM __source("raw_payments")\n  GROUP BY region\n)\n'
        "SELECT total_amount, region FROM base\n"
    ),
}


def _pivot_models(rng: random.Random) -> dict[str, str]:
    """A typed pivot every passthrough reads, and seeded pivot shapes proven or refused."""

    files: dict[str, str] = {
        "models/marts/status_amounts.sql": _TYPED_PIVOT,
        "models/marts/status_passthrough.sql": _TYPED_PIVOT_PASSTHROUGH,
    }
    for index, body in enumerate(rng.sample(_PIVOT_BODIES, k=4)):
        aggregate: str = rng.choice(_PIVOT_AGGREGATES)
        header: str = _PIVOT_HEADER.format(
            pivot=rng.choice(("status", "status", "kind")),
            value=rng.choice(("amount", "amount", "order_id")),
            aggregate=aggregate,
        )
        files[f"models/marts/pivot_{index}.sql"] = header + body.format(aggregate=aggregate) + "\n"
    return files


type _Relation = tuple[str, tuple[str, ...]]
type _Template = Callable[[random.Random, list[_Relation]], tuple[str, str, tuple[str, ...]]]


def _star(rng: random.Random, inputs: list[_Relation]) -> tuple[str, str, tuple[str, ...]]:
    relation, columns = inputs[0]
    return "", f"SELECT * FROM {relation}", columns


def _qualified_star(
    rng: random.Random, inputs: list[_Relation]
) -> tuple[str, str, tuple[str, ...]]:
    (left, left_columns), (right, right_columns) = inputs[0], inputs[-1]
    extra: str = rng.choice(right_columns)
    sql: str = (
        f"SELECT a.*, b.{extra} AS joined_{extra.lower()}, 1 AS flag\n"
        f"FROM {left} a\nJOIN {right} b ON TRUE"
    )
    return "", sql, (*left_columns, f"joined_{extra.lower()}", "flag")


def _nested_ctes(rng: random.Random, inputs: list[_Relation]) -> tuple[str, str, tuple[str, ...]]:
    relation, columns = inputs[0]
    picked: list[str] = rng.sample(list(columns), k=min(2, len(columns)))
    sql: str = (
        f"WITH base AS (\n  SELECT {', '.join(picked)} FROM {relation}\n),\n"
        "layered AS (\n  SELECT * FROM base\n)\n"
        f"SELECT {picked[0]} AS key_value, COUNT(*) AS row_count\nFROM layered\nGROUP BY 1"
    )
    return "", sql, ("key_value", "row_count")


def _set_operation(rng: random.Random, inputs: list[_Relation]) -> tuple[str, str, tuple[str, ...]]:
    (left, left_columns), (right, right_columns) = inputs[0], inputs[-1]
    operator: str = rng.choice(("UNION ALL", "UNION", "EXCEPT", "INTERSECT"))
    sql: str = (
        f"SELECT {left_columns[0]} AS shared_key, 'left' AS side FROM {left}\n"
        f"{operator}\nSELECT {right_columns[0]}, 'right' FROM {right}"
    )
    return "", sql, ("shared_key", "side")


def _expressions(rng: random.Random, inputs: list[_Relation]) -> tuple[str, str, tuple[str, ...]]:
    relation, columns = inputs[0]
    first, last = columns[0], columns[-1]
    sql: str = (
        f"SELECT CAST({first} AS VARCHAR) AS as_text, {first} IS NULL AS is_missing,\n"
        f"  COALESCE({first}, {last}) AS filled, {last} AS passthrough,\n"
        f"  ROW_NUMBER() OVER (PARTITION BY {last} ORDER BY {first}) AS position\n"
        f"FROM {relation}\nWHERE {first} IS NOT NULL"
    )
    return "", sql, ("as_text", "is_missing", "filled", "passthrough", "position")


def _aggregate(rng: random.Random, inputs: list[_Relation]) -> tuple[str, str, tuple[str, ...]]:
    relation, columns = inputs[0]
    column: str = rng.choice(columns)
    sql: str = (
        f"SELECT {column} AS group_key, COUNT(*) AS row_count, MAX({columns[-1]}) AS latest\n"
        f"FROM {relation}\nGROUP BY 1"
    )
    return "", sql, ("group_key", "row_count", "latest")


def _quoted(rng: random.Random, inputs: list[_Relation]) -> tuple[str, str, tuple[str, ...]]:
    relation, columns = inputs[0]
    column: str = rng.choice(columns)
    sql: str = (
        f'SELECT "{column}" AS "Mixed_Case", UPPER(CAST(Src.{column.upper()} AS VARCHAR)) AS Up\n'
        f"FROM {relation} AS Src"
    )
    return "", sql, ("Mixed_Case", "Up")


def _contract(rng: random.Random, inputs: list[_Relation]) -> tuple[str, str, tuple[str, ...]]:
    relation, columns = inputs[0]
    sql: str = (
        f"SELECT CAST({columns[0]} AS INTEGER) AS contract_key,\n"
        f"  CAST({columns[-1]} AS VARCHAR) AS contract_label\nFROM {relation}"
    )
    header: str = (
        "  contract enforced,\n"
        "  columns (contract_key (type INTEGER), contract_label (type VARCHAR)),\n"
    )
    return header, sql, ("contract_key", "contract_label")


_CTE_CONTRACT_BODIES: tuple[str, ...] = (
    "WITH base AS (\n  SELECT {a}, {b} FROM {left}\n)\nSELECT {a}, {b} FROM base",
    "WITH base AS (\n  SELECT CAST({a} AS VARCHAR(12)) AS label, COALESCE({b}, {a}) AS filled,\n"
    "    'fixed' AS tag, CAST(1.5 AS DECIMAL(10, 2)) AS ratio, {a} IS NULL AS missing\n"
    "  FROM {left}\n  WHERE {a} IS NOT NULL\n)\n"
    "SELECT label, filled, tag, ratio, missing FROM base",
    "WITH base AS (\n  SELECT CASE WHEN {a} IS NULL THEN 'none' ELSE 'some' END AS state,\n"
    "    UPPER(CAST({b} AS VARCHAR)) AS upper_b, CAST({a} AS TEXT) || '-x' AS joined\n"
    "  FROM {left}\n)\nSELECT state, upper_b, joined FROM base",
    "WITH unioned AS (\n  SELECT {a} AS shared FROM {left}\n  UNION ALL\n"
    "  SELECT {c} FROM {right}\n)\nSELECT shared FROM unioned",
    "WITH base AS (\n  SELECT * EXCLUDE ({a}) FROM {left}\n)\nSELECT {b} FROM base",
    "WITH lhs AS (\n  SELECT {a}, {b} FROM {left}\n),\nrhs AS (\n  SELECT {c} FROM {right}\n)\n"
    "SELECT l.{a}, r.{c}\nFROM lhs l\nLEFT JOIN rhs r ON l.{a} = r.{c}",
    "WITH outer_cte AS (\n  WITH inner_cte AS (SELECT {a}, {b} FROM {left})\n"
    "  SELECT {a}, {b} FROM inner_cte\n)\nSELECT {a}, {b} FROM outer_cte",
    "WITH base AS (\n  SELECT {a}, NULLIF({b}, {b}) AS cleared, MAX({b}) AS latest FROM {left}\n"
    "  GROUP BY {a}\n)\nSELECT {a}, cleared, latest FROM base WHERE {a} IS NOT NULL",
)


def _cte_contract(rng: random.Random, inputs: list[_Relation]) -> tuple[str, str, tuple[str, ...]]:
    (left, left_columns), (right, right_columns) = inputs[0], inputs[-1]
    first, second = rng.sample(list(left_columns), k=2)
    body: str = rng.choice(_CTE_CONTRACT_BODIES)
    sql: str = body.format(a=first, b=second, c=right_columns[0], left=left, right=right)
    return "  contract enforced,\n", sql, (first, second)


def _unknown_column(
    rng: random.Random, inputs: list[_Relation]
) -> tuple[str, str, tuple[str, ...]]:
    relation, columns = inputs[0]
    sql: str = f"SELECT {columns[0]}, missing_column FROM {relation}"
    return "", sql, (columns[0], "missing_column")


def _cast_constant(rng: random.Random, inputs: list[_Relation]) -> tuple[str, str, tuple[str, ...]]:
    relation, columns = inputs[0]
    column: str = rng.choice(columns)
    sql: str = f"SELECT {column}, CAST(NULL AS INT) AS missing_id\nFROM {relation}"
    return "", sql, (column, "missing_id")


def _subquery(rng: random.Random, inputs: list[_Relation]) -> tuple[str, str, tuple[str, ...]]:
    relation, columns = inputs[0]
    column: str = rng.choice(columns)
    sql: str = f"SELECT s.{column}, s.* FROM (SELECT * FROM {relation}) s"
    return "", sql, columns


_TEMPLATES: tuple[_Template, ...] = (
    _star,
    _qualified_star,
    _nested_ctes,
    _set_operation,
    _expressions,
    _aggregate,
    _quoted,
    _contract,
    _cte_contract,
    _unknown_column,
    _subquery,
    _cast_constant,
)


def pivot_project_files() -> dict[str, str]:
    """A typed pivot with its passthrough and one plain model, for selective assembly."""

    return {
        "sqlbuild_project.toml": _PROJECT_TOML,
        "sources/raw.yml": _SOURCES,
        "models/marts/status_amounts.sql": _TYPED_PIVOT,
        "models/marts/status_passthrough.sql": _TYPED_PIVOT_PASSTHROUGH,
        "models/staging/orders_list.sql": (
            'MODEL (description "Order ids");\n\nSELECT order_id FROM __source("raw_orders")\n'
        ),
    }


@dataclass(frozen=True)
class NativePivotProofs:
    """How many standalone pivot proofs assembly ran, and how many a session proved."""

    proofs: int
    session_proofs: int


def native_pivot_proofs() -> NativePivotProofs:
    """The standalone pivot proofs assembly ran since the counters were last reset."""

    work: dict[str, int] = native_module.compile_stage_work_info(reset=True)
    return NativePivotProofs(
        proofs=work.get("pivot_proofs", 0), session_proofs=work.get("session_pivot_proofs", 0)
    )


def generated_analysis_files(*, rng: random.Random, model_count: int) -> dict[str, str]:
    """Sources (typed, inferred and untyped), a pivot and models drawing on every template."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": _PROJECT_TOML,
        "sources/raw.yml": _SOURCES,
        **_pivot_models(rng),
        **_CONTRACT_CTE_MODELS,
    }
    relations: list[_Relation] = list(_INITIAL_RELATIONS)
    for index in range(model_count):
        template: _Template = _TEMPLATES[index % len(_TEMPLATES)]
        header, sql, columns = template(rng, rng.sample(relations, k=2))
        name: str = f"orders_model_{index}"
        files[f"models/layer_{index % 3}/{name}.sql"] = (
            f'MODEL (\n  description "Generated orders model {index}",\n{header});\n\n{sql}\n'
        )
        relations.append((f'__ref("{name}")', columns))
    return files


def shared_analysis_files(
    *, regions: tuple[str, ...], inexact_regions: tuple[str, ...]
) -> dict[str, str]:
    """Equal regional models and rollups, missing-column readers and one unshared summary."""

    regional: dict[str, str] = {
        f"models/staging/orders_{region}.sql": (
            f'MODEL (description "Orders in {region}");\n\n'
            'SELECT order_id, customer_id, amount FROM __source("raw_orders")\n'
        )
        for region in regions
    }
    rollups: dict[str, str] = {
        f"models/marts/totals_{region}.sql": (
            f'MODEL (description "Customer totals in {region}");\n\n'
            "SELECT customer_id, SUM(amount) AS total_amount\n"
            f'FROM __ref("orders_{region}")\nGROUP BY customer_id\n'
        )
        for region in regions
    }
    inexact: dict[str, str] = {
        f"models/marts/missing_{region}.sql": (
            f'MODEL (description "Orders in {region} with an unknown column");\n\n'
            f'SELECT customer_id, missing_column FROM __ref("orders_{region}")\n'
        )
        for region in inexact_regions
    }
    return {
        "sqlbuild_project.toml": _PROJECT_TOML,
        "sources/raw.yml": _SOURCES,
        **regional,
        **rollups,
        **inexact,
        "models/marts/orders_summary.sql": (
            'MODEL (description "Order count");\n\n'
            f'SELECT COUNT(*) AS order_count FROM __ref("orders_{regions[0]}")\n'
        ),
    }


def compile_inputs(
    *, project_dir: Path, files: dict[str, str], dialect: str | None = "duckdb"
) -> CompileInputsView:
    """Write a project and attach its compile inputs, compiled under `dialect`."""

    for relative_path, contents in files.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")
    return build_compile_inputs(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter_context=CompileAdapterContext(
            value_renderer=DuckDbAdapter(),
            collection_rendering=CollectionRendering.VALUE_LIST,
            python_functions_inherit_default_namespace=True,
            sql_lexical_syntax=DuckDbAdapter.sql_lexical_syntax,
            sql_analysis_dialect=dialect,
        ),
        run_id="integration_run",
    )


def compiled_project_view(
    *,
    project_dir: Path,
    files: dict[str, str],
    capsys: pytest.CaptureFixture[str],
) -> tuple[int, object, object, dict[str, str]]:
    """Compile a written project through the CLI: exit code, report, manifest nodes, SQL files."""

    for relative_path, contents in files.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")
    _ = capsys.readouterr()
    code: int = main(["--project-dir", str(project_dir), "compile", "--json", "--manifest"])
    report: dict[str, object] = json.loads(capsys.readouterr().out)
    manifest: dict[str, object] = json.loads(
        (project_dir / "target" / "manifest.json").read_text(encoding="utf-8")
    )
    compiled: Path = project_dir / "target" / "compiled"
    _ = report.pop("compile_timings")
    _ = manifest.pop("metadata")
    return (
        code,
        report,
        manifest,
        {
            path.relative_to(compiled).as_posix(): path.read_text(encoding="utf-8")
            for path in sorted(compiled.rglob("*.sql"))
        },
    )


@dataclass
class NativeAnalysisRuns:
    """Each assembly's uncached, cold-cached and warm-cached views, and its uncached sessions."""

    names: list[object] = field(default_factory=list)
    uncached: list[object] = field(default_factory=list)
    cached: list[object] = field(default_factory=list)
    analysed_models: int = 0
    expression_shapes: int = 0
    pivot_proofs: int = 0
    standalone_proofs: int = 0
    session_proofs: int = 0
    proven_pivots: int = 0
    native_enrichments: int = 0
    native_column_values: int = 0
    golden: list[GoldenEntry] = field(default_factory=list)
    sessions: list[Any] = field(default_factory=list)


type AssemblyStage = Literal["expression_shapes", "model_facts", "semantic_completion"]


def analyse_natively(
    *,
    inputs: CompileInputsView,
    inference_profile: ExpressionInferenceProfile,
    lineage_mode: ColumnLineageMode,
    runs: NativeAnalysisRuns,
    cache_root: Path,
) -> None:
    """Assemble `inputs` with a cold, then a warm analysis cache, then uncached, and compare them.

    Every analysis, catalog change, session and standalone dynamic pivot proof, and expression
    shape must not depend on the analysis store, on the session, or on Python's shape inference.
    `runs.golden` gathers the entries the uncached assembly records, in the order it records them.
    """

    views: list[object] = []

    def recorded_shapes(
        sink: NativeAnalysisRuns,
        *,
        expressions: tuple[str, ...],
        profile: ExpressionInferenceProfile,
    ) -> None:
        catalog: Any = cast(Any, profile.binding_catalog)
        native: tuple[dict[str, str] | None, ...] = infer_native_expression_source_shapes(
            expressions=expressions, profile=profile
        )
        sink.expression_shapes += sum(shape is not None for shape in native)
        sink.golden.append(golden_entry("shapes", (native, catalog.expression_shapes)))

    def recorded_analyses(
        sink: NativeAnalysisRuns,
        *,
        profile: ExpressionInferenceProfile,
        session: Any,
        tables: NativePivotTables,
    ) -> None:
        _record_analyses(
            inputs=inputs,
            catalog=profile.binding_catalog,
            proof_inputs=(tables, session),
            views=views,
            runs=sink,
        )

    def recorded_proofs(
        sink: NativeAnalysisRuns, *, project: CompiledProjectView, **keywords: Any
    ) -> CompiledProjectView:
        sink.golden.extend(
            golden_entry("proof", model.dynamic_column_contract) for model in project.models
        )
        return native_module.complete_semantic_project(project=project, **keywords)

    def stage_inputs(stage: AssemblyStage, cache_dir: Path | None) -> list[dict[str, Any]]:
        found: dict[str, Any] | None = native_module.assembly_stage_inputs(
            stage,
            inputs=inputs,
            inference_profile=replace(inference_profile),
            column_lineage_mode=lineage_mode,
            analysis_cache_dir=cache_dir,
        )
        return list(filter(None, [found]))

    for cache_dir, sink in (
        (cache_root, NativeAnalysisRuns()),
        (cache_root, NativeAnalysisRuns()),
        (None, runs),
    ):
        with suppress(CompileInputError):
            shapes: list[dict[str, Any]] = stage_inputs("expression_shapes", cache_dir)
            _ = [recorded_shapes(sink, **found) for found in shapes]
            facts: list[dict[str, Any]] = stage_inputs("model_facts", cache_dir)
            _ = [recorded_analyses(sink, **found) for found in facts[: len(shapes)]]
            _ = [
                recorded_proofs(sink, **found)
                for found in stage_inputs("semantic_completion", cache_dir)
            ]
    for index, view in enumerate(views[:-1]):
        _append(runs, f"model analyses, cached run {index}", views[-1], view)


def _record_analyses(
    *,
    inputs: CompileInputsView,
    catalog: object,
    proof_inputs: tuple[NativePivotTables, Any],
    views: list[object],
    runs: NativeAnalysisRuns,
) -> None:
    """Record one assembly's analyses, catalog and session, and compare its pivot proofs."""

    tables, session = proof_inputs
    analysed: list[ModelInput] = list(
        filter(attrgetter("sql_validation_enabled"), inputs.model_inputs)
    )
    sources: list[str] = [session.outcome_rows(position)[0][2] for position in range(len(analysed))]
    runs.native_enrichments += sum(source != LINEAGE_NATIVE for source in sources)
    analyses: dict[str, ModelSqlAnalysis] = {
        model_input.model_file.file_path.stem: session_outcome_analysis(
            session=session, position=position, placeholders=model_placeholders(model_input)
        )
        for position, model_input in enumerate(analysed)
    }
    views.append((_analysis_views(analyses), _catalog_view(catalog)))
    columns: list[InferredColumn] = list(
        chain.from_iterable(map(_native_columns, analyses.values()))
    )
    runs.native_column_values += len(set(columns))
    runs.analysed_models += len(analyses)
    runs.golden.append(golden_entry("models", _analysis_views(analyses)))
    runs.golden.append(golden_entry("catalog", _catalog_view(catalog)))
    _compare_proofs(inputs=inputs, tables=tables, analyses=analyses, session=session, runs=runs)
    runs.sessions.append(session)


def first_model_types(
    *,
    files: dict[str, str],
    lineage_mode: ColumnLineageMode,
    tmp_path: Path,
    dialect: str = "duckdb",
) -> tuple[tuple[str, str | None], ...]:
    """The first analysed model's output names and types, analysed natively under `dialect`."""

    inputs: CompileInputsView = compile_inputs(
        project_dir=tmp_path / "project", files=files, dialect=dialect
    )
    runs: NativeAnalysisRuns = NativeAnalysisRuns()
    analyse_natively(
        inputs=inputs,
        inference_profile=ExpressionInferenceProfile(sql_analysis_dialect=dialect),
        lineage_mode=lineage_mode,
        runs=runs,
        cache_root=tmp_path / "cache",
    )
    analysis: ModelSqlAnalysis = session_outcome_analysis(
        session=runs.sessions[0], position=0, placeholders=None
    )
    return tuple((column.name, column.type) for column in analysis.polyglot_analysis.columns or ())


def custom_nullability_rule(arguments: tuple[InferredNullability, ...]) -> InferredNullability:
    """An adapter rule SQLBuild does not ship, so native analysis calls it back."""

    return InferredNullability.NULLABLE


def _native_columns(analysis: ModelSqlAnalysis) -> tuple[InferredColumn, ...]:
    return analysis.polyglot_analysis.columns or ()


def _compare_proofs(
    *,
    inputs: CompileInputsView,
    tables: NativePivotTables,
    analyses: dict[str, ModelSqlAnalysis],
    session: Any,
    runs: NativeAnalysisRuns,
) -> None:
    """Each analysed pivot model's proof, proven again in the session and standalone."""

    proved: list[bool] = [
        bool(_dynamic_families(model_input)) and model_input.model_file.file_path.stem in analyses
        for model_input in inputs.model_inputs
    ]
    pivot_inputs: list[ModelInput] = list(itertools.compress(inputs.model_inputs, proved))
    for model_input in pivot_inputs:
        families: tuple[SchemaDynamicColumnFamily, ...] = _dynamic_families(model_input)
        analysis: ModelSqlAnalysis = analyses[model_input.model_file.file_path.stem]
        models: tuple[tuple[str, tuple[SchemaDynamicColumnFamily, ...]], ...] = (
            (
                substitute_placeholder_defaults(
                    cursor_intrinsics_analysis_sql(
                        sql=model_input.query_sql,
                        cursor_type=model_input.config.values.get("cursor_type"),
                    ),
                    list((model_placeholders(model_input) or {}).items()),
                ),
                families,
            ),
        )
        proof: DynamicColumnContractProof | None = analysis.dynamic_column_contract
        standalone: DynamicColumnContractProof | None = contract_proof(
            prove_native_dynamic_contracts(session=None, tables=tables, models=models)[0]
        )
        in_session: DynamicColumnContractProof | None = contract_proof(
            prove_native_dynamic_contracts(session=session, tables=tables, models=models)[0]
        )
        runs.pivot_proofs += proof is not None
        runs.proven_pivots += proof is not None and proof.output_proven
        runs.standalone_proofs += standalone is not None
        runs.session_proofs += in_session is not None
        _append(runs, "standalone dynamic pivot proof", proof, standalone)
        _append(runs, "session dynamic pivot proof", proof, in_session)


def _append(runs: NativeAnalysisRuns, name: str, uncached: object, cached: object) -> None:
    runs.names.append(name)
    runs.uncached.append(uncached)
    runs.cached.append(cached)


def _analysis_views(analyses: dict[str, ModelSqlAnalysis]) -> object:
    return {
        name: (
            analysis.polyglot_analysis.analysis_succeeded,
            analysis.polyglot_analysis.columns,
            isinstance(analysis.polyglot_analysis.lineage_columns, CompactLineageFacts),
            tuple(analysis.polyglot_analysis.lineage_columns),
            analysis.polyglot_analysis.has_star,
            analysis.polyglot_analysis.star_resolved,
            analysis.polyglot_analysis.binding_diagnostics,
            analysis.polyglot_analysis.binding_validated,
            analysis.cleaned_sql,
            analysis.placeholders,
        )
        for name, analysis in analyses.items()
    }


def _catalog_view(catalog: Any) -> tuple[object, ...]:
    return (dict(catalog.schemas), dict(catalog.analysis_shapes))


class RecordingNullabilityRule:
    """An adapter nullability rule recording its calls, answering nullable or raising."""

    def __init__(self, *, raised: type[Exception] | None) -> None:
        self.calls: list[tuple[InferredNullability, ...]] = []
        self._outcomes: list[Callable[[], InferredNullability]] = [
            _nullable,
            *(functools.partial(_raise, error) for error in filter(None, (raised,))),
        ]

    def __call__(self, arguments: tuple[InferredNullability, ...]) -> InferredNullability:
        self.calls.append(arguments)
        return self._outcomes[-1]()


def adapter_rule_project(
    *, test_case: AdapterRuleCallbackTestCase, rule: RecordingNullabilityRule, tmp_path: Path
) -> tuple[CompileInputsView, ExpressionInferenceProfile]:
    """A generated project with the adapter-rule models, and a DuckDB profile naming `rule`.

    A native route does not call an adapter's own rules; compile warns instead.
    """

    return (
        compile_inputs(
            project_dir=tmp_path / "project",
            files={
                **generated_analysis_files(
                    rng=random.Random(test_case.seed), model_count=test_case.model_count
                ),
                **ADAPTER_RULE_MODELS,
            },
        ),
        ExpressionInferenceProfile(
            sql_analysis_dialect="duckdb",
            function_nullability_rules={"UPPER": rule, "IFF": rule},
        ),
    )


def _nullable() -> InferredNullability:
    return InferredNullability.NULLABLE


def _raise(error: type[Exception]) -> InferredNullability:
    raise error("adapter rule failed")


def write_shared_query_project(
    *, project_dir: Path, models: dict[str, str], upstream_sql: str
) -> None:
    """Write an orders project with `orders`, `customers` and `models`."""

    _ = (project_dir / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n[rules]\nselect = []\n', encoding="utf-8"
    )
    model_dir: Path = project_dir / "models"
    model_dir.mkdir()
    for name, sql in {"orders": upstream_sql, "customers": upstream_sql, **models}.items():
        _ = (model_dir / f"{name}.sql").write_text(sql, encoding="utf-8")


def upstream_lineage_sources(
    *, project_dir: Path, column: str, mode: str, capsys: pytest.CaptureFixture[str]
) -> tuple[int, list[str]]:
    """The lineage command's exit code and the upstream resources of `column` one level up."""

    code: int = main(
        [
            "--project-dir",
            str(project_dir),
            "lineage",
            column,
            "--format",
            "json",
            "--direction",
            "upstream",
            "--depth",
            "1",
            "--mode",
            mode,
        ]
    )
    trace: list[dict[str, Any]] = json.loads(capsys.readouterr().out)["trace"]
    return code, [step["source"]["resource_name"] for step in trace]


def projection_view(
    row: tuple[list[tuple[str, str]], list[str], list[str]],
) -> tuple[dict[str, str], frozenset[str], frozenset[str]]:
    """A native projection-facts row as Python's `_simple_projection_facts` returned it."""

    types, null_literal_names, quoted_names = row
    return dict(types), frozenset(null_literal_names), frozenset(quoted_names)


def _dynamic_families(model_input: ModelInput) -> tuple[SchemaDynamicColumnFamily, ...]:
    return getattr(model_input.schema_entry, "dynamic_columns", ())
