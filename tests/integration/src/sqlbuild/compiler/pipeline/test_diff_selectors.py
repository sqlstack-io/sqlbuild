from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredProjectInputs,
    build_compiled_project,
    discover_project_inputs,
    resolve_diff_model_names,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import (
    DiffSelectorIntegrationTestCase,
)

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "demo"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        '[connection]\ndatabase = ":memory:"\n\n'
        '[targets.dev]\nschema = "dev_schema"\n'
    ),
    "sql/models/staging/stg_orders.sql": (
        "MODEL (description 'Test model stg_orders.', materialized table\ntags [staging]);\n\nSELECT 1 AS order_id"
    ),
    "sql/models/staging/stg_customers.sql": (
        "MODEL (description 'Test model stg_customers.', materialized table\ntags [staging]);\n\nSELECT 1 AS customer_id"
    ),
    "sql/models/intermediate/int_orders.sql": (
        'MODEL (description "Test model int_orders.", materialized table\ntags [core]);\n\nSELECT order_id FROM __ref("stg_orders")'
    ),
    "sql/models/marts/fact_orders.sql": (
        'MODEL (description "Test model fact_orders.", materialized table\ntags [core, mart]);\n\nSELECT order_id FROM __ref("int_orders")'
    ),
    "sql/models/marts/dim_customers.sql": (
        'MODEL (description "Test model dim_customers.", materialized table\ntags [mart]);\n\nSELECT customer_id FROM __ref("stg_customers")'
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        DiffSelectorIntegrationTestCase(
            description="name selector returns a single model",
            select=("fact_orders",),
            exclude=(),
            expected_model_names=frozenset(("fact_orders",)),
        ),
        DiffSelectorIntegrationTestCase(
            description="path selector returns models under folder",
            select=("path:sql/models/staging",),
            exclude=(),
            expected_model_names=frozenset(("stg_orders", "stg_customers")),
        ),
        DiffSelectorIntegrationTestCase(
            description="Windows-style path selector returns models under folder",
            select=("path:models\\staging",),
            exclude=(),
            expected_model_names=frozenset(("stg_orders", "stg_customers")),
        ),
        DiffSelectorIntegrationTestCase(
            description="tag selector returns tagged models",
            select=("tag:mart",),
            exclude=(),
            expected_model_names=frozenset(("fact_orders", "dim_customers")),
        ),
        DiffSelectorIntegrationTestCase(
            description="upstream expansion includes model ancestors",
            select=("+fact_orders",),
            exclude=(),
            expected_model_names=frozenset(("stg_orders", "int_orders", "fact_orders")),
        ),
        DiffSelectorIntegrationTestCase(
            description="downstream expansion includes model descendants",
            select=("stg_orders+",),
            exclude=(),
            expected_model_names=frozenset(("stg_orders", "int_orders", "fact_orders")),
        ),
        DiffSelectorIntegrationTestCase(
            description="exclude subtracts from selected models",
            select=("tag:mart",),
            exclude=("dim_customers",),
            expected_model_names=frozenset(("fact_orders",)),
        ),
        DiffSelectorIntegrationTestCase(
            description="comma selector intersects resolved sets",
            select=("tag:core,path:sql/models/marts",),
            exclude=(),
            expected_model_names=frozenset(("fact_orders",)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_compiled_project_when_resolving_diff_selectors_then_returns_expected_models(
    test_case: DiffSelectorIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, _PROJECT_FILES)
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discovered_inputs,
        adapter=DuckDbAdapter(),
        selected_target="dev",
        resolved_connection={"database": ":memory:"},
        no_sql_validation=True,
    )

    model_names: tuple[str, ...] = resolve_diff_model_names(
        project=project,
        select=test_case.select,
        exclude=test_case.exclude,
    )

    assert frozenset(model_names) == test_case.expected_model_names
