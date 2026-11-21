"""Literal SQL in Python nodes and hooks must not hard-code project relation names."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.unit.src.sqlbuild.compiler.compile._helpers._test_types import (
    PythonSqlReferenceAllowedTestCase,
    PythonSqlReferenceErrorTestCase,
)
from tests.unit.src.sqlbuild.compiler.compile._helpers.helpers import (
    compile_and_assemble,
    gate_model,
    python_check_source,
    python_hook_source,
    python_loader_source,
    python_task_source,
    render_compile_diagnostics,
)

_PROJECT_FILE: str = """
name = "demo"
adapter = "duckdb"

[settings]
sql_analysis = false
sql_validation = false
"""
_DISABLED_PROJECT_FILE: str = _PROJECT_FILE + "\n[references]\nenforce_explicit = false\n"
_HOOK_PATH: str = "models/marts/_sqlbuild/_hooks/python/lookups.py"
_TASK_PATH: str = "python/tasks/export.py"
_HOOKED_HEADER: str = (
    'MODEL (description "Test model export.", post_hooks [python("refresh_lookup")]);'
)
_MODELS: dict[str, str] = {
    "models/marts/customers.sql": gate_model(sql="SELECT 1 AS customer_id"),
    "models/marts/orders.sql": gate_model(sql="SELECT 1 AS order_id", header=_HOOKED_HEADER),
}
_COUNTRY_SEED: dict[str, str] = {
    "seeds/country_codes.yml": (
        "seeds:\n  - name: country_codes\n    description: Test seed country_codes.\n    columns:\n      - name: code\n        type: VARCHAR\n"
    ),
    "seeds/country_codes.csv": "code\nGB\n",
}


_CHECK_PATH: str = "python/checks/orders.py"
_LOADER_PATH: str = "python/loaders/raw.py"
_RAW_SOURCES: dict[str, str] = {
    "sources/raw.yml": (
        "sources:\n"
        "  - name: raw_regions\n    description: Test source raw_regions.\n    managed: true\n    write_strategy: table\n"
        "    columns:\n      - name: id\n        type: INTEGER\n"
        "  - name: raw_customers\n    description: Test source raw_customers.\n    managed: true\n    write_strategy: table\n"
        "    columns:\n      - name: id\n        type: INTEGER\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    (
        PythonSqlReferenceErrorTestCase(
            description="task literal SQL naming a model",
            files={
                **_MODELS,
                _HOOK_PATH: python_hook_source(),
                _TASK_PATH: python_task_source(
                    body='    ctx.query("SELECT count(*) FROM customers")\n'
                ),
            },
            expected_error_fragments=(
                "[P008]",
                "asset:export_orders names model:customers as 'customers' in SQL passed to "
                "ctx.query()",
                "python/tasks/export.py:7",
                'depends_on=model("customers")',
                "if 'customers' is an external table that shares the name, qualify it with its "
                "schema",
                "[references]\n            enforce_explicit = false",
            ),
        ),
        PythonSqlReferenceErrorTestCase(
            description="every hard-coded relation across Python nodes is reported",
            files={
                **_MODELS,
                _HOOK_PATH: python_hook_source(),
                _TASK_PATH: python_task_source(
                    body=(
                        '    ctx.query("SELECT count(*) FROM customers")\n'
                        '    ctx.query("SELECT * FROM customers JOIN orders ON true")\n'
                    )
                ),
                _CHECK_PATH: python_check_source(
                    depends_on='model("orders")',
                    body='    ctx.query("SELECT count(*) FROM customers")\n',
                ),
            },
            expected_error_fragments=(
                "[P008] check:orders_present names model:customers as 'customers' in SQL passed "
                "to ctx.query() --> python/checks/orders.py:7",
                "[P008] asset:export_orders names model:customers as 'customers' in SQL passed to "
                "ctx.query() --> python/tasks/export.py:7",
                "[P008] asset:export_orders names model:customers as 'customers' in SQL passed to "
                "ctx.query() --> python/tasks/export.py:8",
                "[P008] asset:export_orders names model:orders as 'orders' in SQL passed to "
                "ctx.query() --> python/tasks/export.py:8",
            ),
        ),
        PythonSqlReferenceErrorTestCase(
            description="task literal SQL passed as the sql keyword",
            files={
                **_MODELS,
                _HOOK_PATH: python_hook_source(),
                _TASK_PATH: python_task_source(
                    body='    ctx.query(sql="SELECT count(*) FROM customers")\n'
                ),
            },
            expected_error_fragments=(
                "[P008] asset:export_orders names model:customers as 'customers' in SQL passed to "
                "ctx.query() --> python/tasks/export.py:7",
            ),
        ),
        PythonSqlReferenceErrorTestCase(
            description="hook literal SQL passed to execute_sql as the sql keyword",
            files={
                **_MODELS,
                _HOOK_PATH: python_hook_source(
                    body='    ctx.execute_sql(sql="DELETE FROM customers")\n'
                ),
            },
            expected_error_fragments=(
                "[P008]",
                "names model:customers as 'customers' in SQL passed to ctx.execute_sql()",
            ),
        ),
        PythonSqlReferenceErrorTestCase(
            description="task literal SQL unpacked from a keyword dictionary",
            files={
                **_MODELS,
                _HOOK_PATH: python_hook_source(),
                _TASK_PATH: python_task_source(
                    body='    ctx.query(**{"sql": "SELECT count(*) FROM customers"})\n'
                ),
            },
            expected_error_fragments=(
                "[P008] asset:export_orders names model:customers as 'customers' in SQL passed to "
                "ctx.query()",
            ),
        ),
        PythonSqlReferenceErrorTestCase(
            description="task literal SQL naming a seed",
            files={
                **_MODELS,
                **_COUNTRY_SEED,
                _HOOK_PATH: python_hook_source(),
                _TASK_PATH: python_task_source(
                    body='    ctx.query("SELECT * FROM country_codes")\n'
                ),
            },
            expected_error_fragments=(
                "asset:export_orders names seed:country_codes",
                'declare it with depends_on=seed("country_codes") and use '
                'ctx.relation(seed("country_codes"))',
            ),
        ),
        PythonSqlReferenceErrorTestCase(
            description="check literal SQL naming an undeclared model",
            files={
                **_MODELS,
                _HOOK_PATH: python_hook_source(),
                _CHECK_PATH: python_check_source(
                    depends_on='model("orders")',
                    body='    ctx.query("SELECT count(*) FROM customers")\n',
                ),
            },
            expected_error_fragments=(
                "[P008]",
                "check:orders_present names model:customers as 'customers' in SQL passed to "
                "ctx.query()",
                "python/checks/orders.py:7",
                'declare it with depends_on=model("customers")',
            ),
        ),
        PythonSqlReferenceErrorTestCase(
            description="loader literal SQL naming a source it does not depend on",
            files={
                **_MODELS,
                **_RAW_SOURCES,
                _HOOK_PATH: python_hook_source(),
                _LOADER_PATH: python_loader_source(
                    depends_on="", body='    ctx.query("SELECT * FROM raw_regions")\n'
                ),
            },
            expected_error_fragments=(
                "loader:raw_customers names source:raw_regions as 'raw_regions'",
                'use ctx.source("raw_regions") instead of the relation name',
            ),
        ),
        PythonSqlReferenceErrorTestCase(
            description="loader literal SQL naming an upstream loader's source",
            files={
                **_MODELS,
                **_RAW_SOURCES,
                _HOOK_PATH: python_hook_source(),
                _LOADER_PATH: python_loader_source(
                    depends_on="raw_regions", body='    ctx.query("SELECT * FROM raw_regions")\n'
                ),
            },
            expected_error_fragments=("use ctx.loader(raw_regions) instead of the relation name",),
        ),
        PythonSqlReferenceErrorTestCase(
            description="loader literal SQL naming a model",
            files={
                **_MODELS,
                **_RAW_SOURCES,
                _HOOK_PATH: python_hook_source(),
                _LOADER_PATH: python_loader_source(
                    depends_on="", body='    ctx.query("SELECT * FROM customers")\n'
                ),
            },
            expected_error_fragments=(
                "loader:raw_customers names model:customers as 'customers'",
                "loaders run before models and seeds and cannot read them; read model:customers "
                "from an asset instead",
            ),
        ),
        PythonSqlReferenceErrorTestCase(
            description="hook f-string with a literal seed table part",
            files={
                **_MODELS,
                **_COUNTRY_SEED,
                _HOOK_PATH: python_hook_source(
                    body=(
                        "    limit = 10\n"
                        '    ctx.execute_sql(f"SELECT * FROM country_codes LIMIT {limit}")\n'
                    )
                ),
            },
            expected_error_fragments=(
                "hook:refresh_lookup names seed:country_codes",
                "ctx.execute_sql()",
                '@hook(reads=seed("country_codes"))',
            ),
        ),
        PythonSqlReferenceErrorTestCase(
            description="hook literal SQL naming a schema-qualified model",
            files={
                **_MODELS,
                _HOOK_PATH: python_hook_source(
                    body='    ctx.query("SELECT * FROM main.customers")\n'
                ),
            },
            expected_error_fragments=("names model:customers as 'main.customers'",),
        ),
        PythonSqlReferenceErrorTestCase(
            description="qualified name omits the external table help",
            files={
                **_MODELS,
                _HOOK_PATH: python_hook_source(
                    body='    ctx.query("SELECT * FROM main.customers")\n'
                ),
            },
            expected_error_fragments=("names model:customers",),
            unexpected_error_fragments=("external table",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_hard_coded_project_relation_when_compiling_then_it_reports_p008(
    test_case: PythonSqlReferenceErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, {"sqlbuild_project.toml": _PROJECT_FILE} | test_case.files)

    rendered: str = render_compile_diagnostics(project=compile_and_assemble(project_dir=tmp_path))

    assert all(fragment in rendered for fragment in test_case.expected_error_fragments)
    assert not any(fragment in rendered for fragment in test_case.unexpected_error_fragments)


@pytest.mark.parametrize(
    "test_case",
    (
        PythonSqlReferenceAllowedTestCase(
            description="non-project relations and a temporary table created by the hook",
            files={
                **_MODELS,
                _HOOK_PATH: python_hook_source(
                    body=(
                        '    ctx.execute_sql("CREATE TEMP TABLE customers AS SELECT 1 AS id")\n'
                        '    ctx.query("SELECT * FROM customers")\n'
                        '    ctx.query("SELECT * FROM information_schema.tables")\n'
                    )
                ),
            },
            expected_model_names=("customers", "orders"),
        ),
        PythonSqlReferenceAllowedTestCase(
            description="relation formatted from ctx.relation and a CTE named like a model",
            files={
                **_MODELS,
                _HOOK_PATH: python_hook_source(),
                _TASK_PATH: python_task_source(
                    body=(
                        '    orders = ctx.relation(model("orders"))\n'
                        '    ctx.query(f"SELECT * FROM {orders}")\n'
                        '    ctx.query("WITH customers AS (SELECT 1 AS id) SELECT * FROM customers")\n'
                    )
                ),
            },
            expected_model_names=("customers", "orders"),
        ),
        PythonSqlReferenceAllowedTestCase(
            description="hook naming the model it runs on",
            files={
                **_MODELS,
                _HOOK_PATH: python_hook_source(body='    ctx.query("SELECT * FROM orders")\n'),
            },
            expected_model_names=("customers", "orders"),
        ),
        PythonSqlReferenceAllowedTestCase(
            description="check reading its declared model through ctx.relation",
            files={
                **_MODELS,
                _HOOK_PATH: python_hook_source(),
                _CHECK_PATH: python_check_source(
                    depends_on='model("orders")',
                    body=(
                        '    orders = ctx.relation(model("orders"))\n'
                        '    ctx.query(f"SELECT count(*) FROM {orders}")\n'
                    ),
                ),
            },
            expected_model_names=("customers", "orders"),
        ),
        PythonSqlReferenceAllowedTestCase(
            description="loader naming its own source and reading upstream through ctx.loader",
            files={
                **_MODELS,
                **_RAW_SOURCES,
                _HOOK_PATH: python_hook_source(),
                _LOADER_PATH: python_loader_source(
                    depends_on="raw_regions",
                    body=(
                        "    regions = ctx.loader(raw_regions)\n"
                        '    ctx.query(f"SELECT * FROM {regions.destination}")\n'
                        '    ctx.query("SELECT count(*) FROM raw_customers")\n'
                    ),
                ),
            },
            expected_model_names=("customers", "orders"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_explicit_or_non_project_relations_when_compiling_then_it_succeeds(
    test_case: PythonSqlReferenceAllowedTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, {"sqlbuild_project.toml": _PROJECT_FILE} | test_case.files)

    project: CompiledProjectView = compile_and_assemble(project_dir=tmp_path)

    assert tuple(sorted(model.name for model in project.models)) == test_case.expected_model_names


@pytest.mark.parametrize(
    "test_case",
    (
        PythonSqlReferenceAllowedTestCase(
            description="enforcement disabled allows hard-coded names",
            files={
                **_MODELS,
                _HOOK_PATH: python_hook_source(),
                _TASK_PATH: python_task_source(
                    body='    ctx.query("SELECT count(*) FROM customers")\n'
                ),
            },
            expected_model_names=("customers", "orders"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_enforcement_disabled_when_compiling_hard_coded_names_then_it_succeeds(
    test_case: PythonSqlReferenceAllowedTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, {"sqlbuild_project.toml": _DISABLED_PROJECT_FILE} | test_case.files)

    project: CompiledProjectView = compile_and_assemble(project_dir=tmp_path)

    assert tuple(sorted(model.name for model in project.models)) == test_case.expected_model_names


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
