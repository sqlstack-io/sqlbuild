"""Python hooks that declare reads order their model after them and cannot read its dependants."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.unit.src.sqlbuild.compiler.compile._helpers._test_types import (
    HookReadEdgeTestCase,
    HookReadErrorTestCase,
)
from tests.unit.src.sqlbuild.compiler.compile._helpers.helpers import (
    compile_and_assemble,
    execution_edge_names,
    gate_model,
    inline_sql_hook_header,
    lineage_edge_names,
    python_hook_source,
    render_compile_diagnostics,
)

_PROJECT_FILE: str = """
name = "demo"
adapter = "duckdb"

[settings]
sql_analysis = false
sql_validation = false
"""
_HOOK_PATH: str = "sql/models/marts/_sqlbuild/_hooks/python/lookups.py"
_HOOKED_HEADER: str = (
    'MODEL (description "Test model lookups.", post_hooks [python("refresh_lookup")]);'
)
_COUNTRY_SEED: str = "seeds:\n  - name: country_codes\n    description: Test seed country_codes.\n    columns:\n      - name: code\n        type: VARCHAR\n"
_SOURCES: str = (
    "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n    table: orders\n"
)
_SQL_HOOK_PATH: str = "sql/models/marts/_sqlbuild/_hooks/sql/record_rows.sql"
_MACRO_PATH: str = "sql/models/marts/_sqlbuild/_macros/python/counts.py"
_IGNORING_MACRO: str = "def count_rows(relation):\n    return 'SELECT 1'\n"
_NAMED_SQL_HOOK_HEADER: str = (
    'MODEL (description "Test model counts.", post_hooks [sql("record_rows")]);'
)


@pytest.mark.parametrize(
    "test_case",
    (
        HookReadEdgeTestCase(
            description="hook reading a model makes the hooked model wait for it",
            files={
                _HOOK_PATH: python_hook_source(reads='model("customers")'),
                "sql/models/marts/customers.sql": gate_model(sql="SELECT 1 AS customer_id"),
                "sql/models/marts/orders.sql": gate_model(
                    sql="SELECT 1 AS order_id", header=_HOOKED_HEADER
                ),
            },
            expected_edges=(("orders", "customers"),),
        ),
        HookReadEdgeTestCase(
            description="hook reading a list of a seed and a source adds one edge per read",
            files={
                _HOOK_PATH: python_hook_source(
                    reads='[seed("country_codes"), source("raw_orders")]'
                ),
                "sql/seeds/country_codes.yml": _COUNTRY_SEED,
                "sql/seeds/country_codes.csv": "code\nGB\n",
                "sql/sources/raw.yml": _SOURCES,
                "sql/models/marts/orders.sql": gate_model(
                    sql="SELECT 1 AS order_id", header=_HOOKED_HEADER
                ),
            },
            expected_edges=(("orders", "country_codes"), ("orders", "raw_orders")),
        ),
        HookReadEdgeTestCase(
            description="inline SQL hook reading a model directly",
            files={
                "sql/models/marts/customers.sql": gate_model(sql="SELECT 1 AS customer_id"),
                "sql/models/marts/orders.sql": gate_model(
                    sql="SELECT 1 AS order_id",
                    header=inline_sql_hook_header('SELECT count(*) FROM __ref("customers")'),
                ),
            },
            expected_edges=(("orders", "customers"),),
        ),
        HookReadEdgeTestCase(
            description="named SQL hook passing a model to a macro that ignores it",
            files={
                _MACRO_PATH: _IGNORING_MACRO,
                _SQL_HOOK_PATH: 'HOOK (description "Test hook.");\n@py.count_rows(__ref("customers"))\n',
                "sql/models/marts/customers.sql": gate_model(sql="SELECT 1 AS customer_id"),
                "sql/models/marts/orders.sql": gate_model(
                    sql="SELECT 1 AS order_id", header=_NAMED_SQL_HOOK_HEADER
                ),
            },
            expected_edges=(("orders", "customers"),),
        ),
        HookReadEdgeTestCase(
            description="inline SQL hook reading a source and a seed",
            files={
                "sql/seeds/country_codes.yml": _COUNTRY_SEED,
                "sql/seeds/country_codes.csv": "code\nGB\n",
                "sql/sources/raw.yml": _SOURCES,
                "sql/models/marts/orders.sql": gate_model(
                    sql="SELECT 1 AS order_id",
                    header=inline_sql_hook_header(
                        'SELECT * FROM __source("raw_orders"), __seed("country_codes")'
                    ),
                ),
            },
            expected_edges=(("orders", "country_codes"), ("orders", "raw_orders")),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_hook_reads_when_compiling_then_hooked_model_runs_after_reads(
    test_case: HookReadEdgeTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, {"sqlbuild_project.toml": _PROJECT_FILE} | test_case.files)

    project: CompiledProjectView = compile_and_assemble(project_dir=tmp_path)

    assert set(test_case.expected_edges) <= execution_edge_names(project=project)


@pytest.mark.parametrize(
    "test_case",
    (
        HookReadEdgeTestCase(
            description="inline SQL hook read orders the build but is not lineage",
            files={
                "sql/models/marts/customers.sql": gate_model(sql="SELECT 1 AS customer_id"),
                "sql/models/marts/orders.sql": gate_model(
                    sql="SELECT 1 AS order_id",
                    header=inline_sql_hook_header('SELECT count(*) FROM __ref("customers")'),
                ),
            },
            expected_edges=(("orders", "customers"),),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_sql_hook_read_when_compiling_then_lineage_and_selection_ignore_it(
    test_case: HookReadEdgeTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, {"sqlbuild_project.toml": _PROJECT_FILE} | test_case.files)

    project: CompiledProjectView = compile_and_assemble(project_dir=tmp_path)

    assert set(test_case.expected_edges) <= execution_edge_names(project=project)
    assert set(test_case.expected_edges).isdisjoint(lineage_edge_names(project=project))


@pytest.mark.parametrize(
    "test_case",
    (
        HookReadErrorTestCase(
            description="hook reading a model built from its own model",
            files={
                _HOOK_PATH: python_hook_source(reads='model("order_rollup")'),
                "sql/models/marts/orders.sql": gate_model(
                    sql="SELECT 1 AS order_id", header=_HOOKED_HEADER
                ),
                "sql/models/marts/order_rollup.sql": gate_model(
                    sql='SELECT count(*) AS n FROM __ref("orders")'
                ),
            },
            expected_error_fragments=(
                "[P007]",
                "Python hook 'refresh_lookup' on model 'orders' reads model 'order_rollup', "
                "which depends on 'orders'",
            ),
        ),
        HookReadErrorTestCase(
            description="every hook read of its own dependant is reported",
            files={
                _HOOK_PATH: python_hook_source(
                    reads='[model("order_rollup"), model("order_count")]'
                ),
                "sql/models/marts/orders.sql": gate_model(
                    sql="SELECT 1 AS order_id", header=_HOOKED_HEADER
                ),
                "sql/models/marts/order_rollup.sql": gate_model(
                    sql='SELECT count(*) AS n FROM __ref("orders")'
                ),
                "sql/models/marts/order_count.sql": gate_model(
                    sql='SELECT count(*) AS n FROM __ref("orders")'
                ),
            },
            expected_error_fragments=(
                "[P007] Python hook 'refresh_lookup' on model 'orders' reads model 'order_count'",
                "[P007] Python hook 'refresh_lookup' on model 'orders' reads model 'order_rollup'",
            ),
        ),
        HookReadErrorTestCase(
            description="inline and named SQL hooks reading a model built from their model",
            files={
                _MACRO_PATH: _IGNORING_MACRO,
                _SQL_HOOK_PATH: 'HOOK (description "Test hook.");\n@py.count_rows(__ref("order_count"))\n',
                "sql/models/marts/orders.sql": gate_model(
                    sql="SELECT 1 AS order_id",
                    header=(
                        "MODEL (description 'Test model orders.', pre_hooks [inline_sql('SELECT * FROM __ref(\"order_rollup\")')], "
                        'post_hooks [sql("record_rows")]);'
                    ),
                ),
                "sql/models/marts/order_rollup.sql": gate_model(
                    sql='SELECT count(*) AS n FROM __ref("orders")'
                ),
                "sql/models/marts/order_count.sql": gate_model(
                    sql='SELECT count(*) AS n FROM __ref("orders")'
                ),
            },
            expected_error_fragments=(
                "[P007] SQL hook 'record_rows' on model 'orders' reads model 'order_count'",
                "[P007] inline SQL hook pre_hooks[0] on model 'orders' reads model "
                "'order_rollup', which depends on 'orders'",
                "use @@CTX:destination.qualified for 'orders' itself",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_hook_reading_its_own_dependant_when_compiling_then_it_reports_p007(
    test_case: HookReadErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, {"sqlbuild_project.toml": _PROJECT_FILE} | test_case.files)

    rendered: str = render_compile_diagnostics(project=compile_and_assemble(project_dir=tmp_path))

    assert all(fragment in rendered for fragment in test_case.expected_error_fragments)


@pytest.mark.parametrize(
    "test_case",
    (
        HookReadErrorTestCase(
            description="hook reading an unknown model",
            files={
                _HOOK_PATH: python_hook_source(reads='model("missing_customers")'),
                "sql/models/marts/orders.sql": gate_model(
                    sql="SELECT 1 AS order_id", header=_HOOKED_HEADER
                ),
            },
            expected_error_fragments=(
                "Python hook 'refresh_lookup'",
                "reads unknown model 'missing_customers'",
            ),
        ),
        HookReadErrorTestCase(
            description="inline SQL hook reading an unknown model",
            files={
                "sql/models/marts/orders.sql": gate_model(
                    sql="SELECT 1 AS order_id",
                    header=inline_sql_hook_header('SELECT * FROM __ref("missing_customers")'),
                ),
            },
            expected_error_fragments=("references unknown model 'missing_customers'",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unsatisfiable_hook_reads_when_compiling_then_it_fails(
    test_case: HookReadErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, {"sqlbuild_project.toml": _PROJECT_FILE} | test_case.files)

    with pytest.raises(ValueError) as error:
        compile_and_assemble(project_dir=tmp_path)

    rendered: str = f"[{getattr(error.value, 'code', '')}] {error.value}"
    assert all(fragment in rendered for fragment in test_case.expected_error_fragments)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
