from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING

import pytest

from tests.unit.src.sqlbuild.compiler.compile._helpers.config._test_types import (
    AttachedColumnsTestCase,
)
from tests.unit.src.sqlbuild.compiler.compile._helpers.config.helpers import (
    compile_input_schema_columns,
    compile_with_native_config,
)

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView


@pytest.mark.parametrize(
    "test_case",
    [
        AttachedColumnsTestCase(
            description="static configuration is shared without aliasing values",
            expected_columns=(
                ("customer_id", "INTEGER", False, "Customer id"),
                ("order_id", "BIGINT", True, "Order id"),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_static_column_contracts_when_attaching_then_values_are_not_aliased(
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    test_case: AttachedColumnsTestCase,
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[settings]
sql_analysis = false
sql_validation = false

[defaults]
materialized = "table"
contract = "enforced"

[materialization_defaults.table]
time_travel_retention = "14d"
""".strip()
            + "\n",
            "sql/models/customers.sql": """
MODEL (description "Test model customers.",
  columns (customer_id (type INTEGER, nullable false, description "Customer id")),
);

SELECT 1 AS customer_id
""".strip()
            + "\n",
            "sql/models/orders.sql": """
MODEL (description "Test model orders.",
  columns (order_id (type BIGINT, nullable true, description "Order id")),
);

SELECT 1 AS order_id
""".strip()
            + "\n",
        },
    )

    compile_inputs: CompileInputsView = compile_with_native_config(project_dir=tmp_path)

    assert (
        tuple(
            (column.name, column.type, column.nullable, column.description)
            for column in compile_input_schema_columns(compile_inputs)
        )
        == test_case.expected_columns
    )
    assert all(
        model_input.config.time_travel_retention.desired_days == 14
        for model_input in compile_inputs.model_inputs
    )
    first_values: dict[str, object] = compile_inputs.model_inputs[0].config.values
    second_values: dict[str, object] = compile_inputs.model_inputs[1].config.values
    assert first_values is not second_values
    first_values["test_marker"] = "first"
    assert "test_marker" not in second_values


@pytest.mark.parametrize(
    "test_case",
    [
        AttachedColumnsTestCase(
            description="templated configurations remain distinct",
            expected_columns=(
                ("INTEGER", "customers identifier"),
                ("INTEGER", "orders identifier"),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_templated_columns_when_attaching_then_resolves_each_model_context(
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    test_case: AttachedColumnsTestCase,
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[settings]
sql_analysis = false
sql_validation = false

[vars]
identifier_type = "INTEGER"
""".strip()
            + "\n",
            "sql/models/customers.sql": """
MODEL (description "Test model customers.",
  columns (
    customer_id (
      type ${identifier_type},
      description "${CTX:model.name} identifier",
    )
  ),
);

SELECT 1 AS customer_id
""".strip()
            + "\n",
            "sql/models/orders.sql": """
MODEL (description "Test model orders.",
  columns (
    order_id (
      type ${identifier_type},
      description "${CTX:model.name} identifier",
    )
  ),
);

SELECT 1 AS order_id
""".strip()
            + "\n",
        },
    )

    compile_inputs: CompileInputsView = compile_with_native_config(project_dir=tmp_path)

    assert (
        tuple(
            (column.type, column.description)
            for column in compile_input_schema_columns(compile_inputs)
        )
        == test_case.expected_columns
    )
