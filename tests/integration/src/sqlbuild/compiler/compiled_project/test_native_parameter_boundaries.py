"""Adapter literals and quoted hook arguments cross real native attachment boundaries."""

from __future__ import annotations

import json
from pathlib import Path
from typing import TYPE_CHECKING, cast

import pytest

from sqlbuild._native import (
    CollectionRendering,
    CompileAdapterContext,
    SqlHookEntry,
    build_compile_inputs,
    discover_project_inputs,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.sqlserver.classes.sqlserver_adapter import SqlServerAdapter
from tests.integration.src.sqlbuild.compiler.compiled_project._test_types import (
    AdapterParameterBoundaryTestCase,
    QuotedHookBoundaryTestCase,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView


@pytest.mark.parametrize(
    "test_case",
    [
        AdapterParameterBoundaryTestCase(
            "decimal scale in BigQuery",
            "bigquery",
            "decimal",
            '"12.3400"',
            "SELECT NUMERIC '12.3400' AS value",
        ),
        AdapterParameterBoundaryTestCase(
            "Unicode literal in SQL Server",
            "sqlserver",
            "string",
            '"O\'Brien"',
            "SELECT N'O''Brien' AS value",
        ),
        AdapterParameterBoundaryTestCase(
            "boolean scalar in SQL Server", "sqlserver", "boolean", "true", "SELECT 1 AS value"
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_adapter_parameter_when_attaching_native_test_then_actual_sql_uses_adapter_literal(
    test_case: AdapterParameterBoundaryTestCase,
    tmp_path: Path,
) -> None:
    adapters: dict[str, BaseAdapter] = {
        "bigquery": BigQueryAdapter(),
        "sqlserver": SqlServerAdapter(),
    }
    adapter: BaseAdapter = adapters[test_case.adapter_name]
    write_files(
        project_dir=tmp_path,
        files={
            "sqlbuild_project.toml": f'name = "orders"\nadapter = "{test_case.adapter_name}"\n',
            "models/raw_orders.sql": 'MODEL (description "Raw orders");\nSELECT 1 AS value\n',
            "models/orders.sql": 'MODEL (description "Orders");\nSELECT value FROM __ref("raw_orders")\n',
            "tests/unit/order_values.sql": (
                f"TEST (parameters (value {test_case.declaration}), cases (single (value {test_case.literal})));\n"
                'WITH __ref__raw_orders AS (SELECT @param("value") AS value), '
                '__expected__orders AS (SELECT @param("value") AS value) SELECT 1\n'
            ),
        },
    )
    inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter_context=CompileAdapterContext(
            value_renderer=adapter,
            collection_rendering=CollectionRendering.VALUE_LIST,
            python_functions_inherit_default_namespace=True,
            sql_lexical_syntax=adapter.sql_lexical_syntax,
            sql_analysis_dialect=adapter.sql_analysis_dialect_name,
        ),
        run_id="test_run",
    )
    assert len(inputs.test_inputs) == 1
    assert test_case.expected_sql in inputs.test_inputs[0].sql_body
    assert '@param("value")' not in inputs.test_inputs[0].sql_body


@pytest.mark.parametrize(
    "test_case",
    [
        QuotedHookBoundaryTestCase(
            "quoted apostrophe and argument marker", "O'Brien@orders", "SELECT 'O''Brien@orders'"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_quoted_hook_argument_when_attaching_then_inserted_marker_is_not_rescanned(
    test_case: QuotedHookBoundaryTestCase,
    tmp_path: Path,
) -> None:
    write_files(
        project_dir=tmp_path,
        files={
            "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
            "hooks/sql/notify.sql": "HOOK (description \"Record a label\");\nSELECT @'label'\n",
            "models/orders.sql": (
                f'MODEL (description "Orders", pre_hooks [sql("notify", label: {json.dumps(test_case.argument)})]);\nSELECT 1 AS order_id\n'
            ),
        },
    )
    inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter_context=CompileAdapterContext(
            value_renderer=DuckDbAdapter(),
            collection_rendering=CollectionRendering.VALUE_LIST,
            python_functions_inherit_default_namespace=True,
            sql_lexical_syntax=DuckDbAdapter.sql_lexical_syntax,
            sql_analysis_dialect=DuckDbAdapter.sql_analysis_dialect_name,
        ),
        run_id="test_run",
    )
    hooks: list[SqlHookEntry] = cast(
        list[SqlHookEntry], inputs.model_inputs[0].config.values["pre_hooks"]
    )
    assert len(hooks) == 1
    assert hooks[0].statement.strip() == test_case.expected_sql
