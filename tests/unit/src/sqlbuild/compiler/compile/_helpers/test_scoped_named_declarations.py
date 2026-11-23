"""Scope placement and visibility for audits, reusable schemas, and named hooks."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import ScopeDiagnosticSeverity as DiagnosticSeverity
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.unit.src.sqlbuild.compiler.compile._helpers._test_types import (
    NamedDeclarationAcceptedTestCase,
    NamedDeclarationErrorTestCase,
    NamedDeclarationWarningTestCase,
)
from tests.unit.src.sqlbuild.compiler.compile._helpers.helpers import (
    compile_and_assemble,
    model_header,
    render_compile_diagnostics,
    singular_audit_files,
)

_PROJECT_FILE: str = """
name = "demo"
adapter = "duckdb"

[settings]
sql_analysis = false
sql_validation = false
"""
_ADVISORY_PROJECT_FILE: str = _PROJECT_FILE + "\n[scopes]\nenforce_placement = false\n"
_ORDERS: str = "MODEL (description 'Test model.');\nSELECT 1 AS order_id"
_CUSTOMERS: str = "MODEL (description 'Test model.');\nSELECT 1 AS order_id"
_SOURCES: str = (
    "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n    table: orders\n"
)
_SEED_YAML: str = "seeds:\n  - name: order_codes\n    description: Test seed order_codes.\n    columns:\n      - name: order_id\n        type: INTEGER\n"
_TABLE_FUNCTION: str = "FUNCTION (description 'Test function.', returns table (order_id INTEGER));\nSELECT 1 AS order_id"
_GENERIC_AUDIT: str = 'AUDIT ();\nSELECT * FROM __ref("@model") WHERE order_id IS NULL'
_SCHEMA: str = "SCHEMA (name order_shape, columns (order_id (type INTEGER)));"
_SQL_HOOK: str = "HOOK (description 'Test hook.');\nSELECT 1"
_PYTHON_HOOK: str = "from sqlbuild.hooks import hook\n\n\n@hook\ndef mark(ctx):\n    '''Test hook mark.'''\n    return None\n"
_SINGULAR_BASE_FILES: dict[str, str] = {
    "sql/models/orders.sql": _ORDERS,
    "sql/models/customers.sql": _CUSTOMERS,
    "sql/sources/raw.yml": _SOURCES,
    "sql/seeds/codes.yml": _SEED_YAML,
    "sql/seeds/order_codes.csv": "order_id\n1\n",
    "sql/functions/sql/order_rows.sql": _TABLE_FUNCTION,
}


@pytest.mark.parametrize(
    "test_case",
    (
        NamedDeclarationAcceptedTestCase(
            "singular audit over two models",
            singular_audit_files(
                base=_SINGULAR_BASE_FILES,
                sql='SELECT * FROM __ref("orders") JOIN __ref("customers") USING (order_id)',
            ),
        ),
        NamedDeclarationAcceptedTestCase(
            "singular audit over a model and a source",
            singular_audit_files(
                base=_SINGULAR_BASE_FILES,
                sql='SELECT * FROM __ref("orders") JOIN __source("raw_orders") USING (id)',
            ),
        ),
        NamedDeclarationAcceptedTestCase(
            "singular audit over a model and a seed",
            singular_audit_files(
                base=_SINGULAR_BASE_FILES,
                sql='SELECT * FROM __ref("orders") JOIN __seed("order_codes") USING (order_id)',
            ),
        ),
        NamedDeclarationAcceptedTestCase(
            "singular audit over a model and a table function",
            singular_audit_files(
                base=_SINGULAR_BASE_FILES,
                sql='SELECT * FROM __ref("orders") JOIN __table_fn("order_rows")() USING (order_id)',
            ),
        ),
        NamedDeclarationAcceptedTestCase(
            "top-level singular audit over a sub-folder model and a table function",
            {
                "sql/models/marts/orders.sql": _ORDERS,
                "sql/functions/sql/order_rows.sql": _TABLE_FUNCTION,
                "sql/audits/singular/check.sql": (
                    'AUDIT ();\nSELECT * FROM __ref("orders") '
                    'JOIN __table_fn("order_rows")() USING (order_id)'
                ),
            },
        ),
        NamedDeclarationAcceptedTestCase(
            "top-level singular audit over a sub-folder model and a source",
            {
                "sql/models/marts/orders.sql": _ORDERS,
                "sql/sources/raw.yml": _SOURCES,
                "sql/audits/singular/check.sql": (
                    'AUDIT ();\nSELECT * FROM __ref("orders") '
                    'JOIN __source("raw_orders") USING (order_id)'
                ),
            },
        ),
        NamedDeclarationAcceptedTestCase(
            "top-level generic audit used across one resource tree",
            {
                "sql/audits/generic/order_check.sql": _GENERIC_AUDIT,
                "sql/models/marts/orders.sql": model_header(key="audits", value="[order_check]"),
                "sql/models/staging/orders_stage.sql": model_header(
                    key="audits", value="[order_check]"
                ),
            },
        ),
        NamedDeclarationAcceptedTestCase(
            "folder-only generic audit used by its folder",
            {
                "sql/models/marts/_sqlbuild/_audits/generic/order_check.sql": _GENERIC_AUDIT,
                "sql/models/marts/orders.sql": model_header(key="audits", value="[order_check]"),
            },
        ),
        NamedDeclarationAcceptedTestCase(
            "inherited generic audit used below its owner",
            {
                "sql/models/marts/_sqlbuild/audits/generic/order_check.sql": _GENERIC_AUDIT,
                "sql/models/marts/daily/orders.sql": model_header(
                    key="audits", value="[order_check]"
                ),
                "sql/models/marts/weekly/orders_weekly.sql": model_header(
                    key="audits", value="[order_check]"
                ),
            },
        ),
        NamedDeclarationAcceptedTestCase(
            "scoped singular audit references resources below its owner",
            {
                "sql/models/marts/orders.sql": _ORDERS,
                "sql/models/marts/customers.sql": _CUSTOMERS,
                "sql/models/marts/_sqlbuild/audits/singular/check.sql": (
                    'AUDIT ();\nSELECT * FROM __ref("orders") JOIN __ref("customers") '
                    "USING (order_id)"
                ),
            },
        ),
        NamedDeclarationAcceptedTestCase(
            "audit factory is the only consumer of a scoped generic audit",
            {
                "sql/models/marts/_sqlbuild/_audits/generic/order_check.sql": _GENERIC_AUDIT,
                "sql/models/marts/orders.sql": model_header(
                    key="audit_factories", value="[order_quality]"
                ),
                "python/nodes/factories/quality.py": (
                    "from sqlbuild.audits import AuditCase, audit_factory\n\n\n"
                    "@audit_factory\ndef order_quality():\n"
                    '    return [AuditCase(name="order_ok", definition="order_check")]\n'
                ),
            },
        ),
        NamedDeclarationAcceptedTestCase(
            "scoped schema, SQL hook, and Python hook bound below their owner",
            {
                "sql/models/marts/_sqlbuild/schemas/order_shape.sql": _SCHEMA,
                "sql/models/marts/_sqlbuild/hooks/sql/touch.sql": _SQL_HOOK,
                "sql/models/marts/_sqlbuild/hooks/python/lifecycle.py": _PYTHON_HOOK,
                "sql/models/marts/daily/orders.sql": (
                    'MODEL (model_schema order_shape, pre_hooks [sql("touch")], '
                    'post_hooks [python("mark")]);\nSELECT 1 AS order_id'
                ),
                "sql/models/marts/weekly/orders_weekly.sql": (
                    'MODEL (model_schema order_shape, pre_hooks [sql("touch")], '
                    'post_hooks [python("mark")]);\nSELECT 1 AS order_id'
                ),
            },
        ),
        NamedDeclarationAcceptedTestCase(
            "child schema extends a parent visible from the child's folder",
            {
                "sql/models/marts/_sqlbuild/schemas/order_shape.sql": _SCHEMA,
                "sql/models/marts/daily/_sqlbuild/_schemas/daily_shape.sql": (
                    "SCHEMA (name daily_shape, extends order_shape, "
                    "columns (order_date (type DATE)));"
                ),
                "sql/models/marts/daily/orders.sql": (
                    "MODEL (model_schema daily_shape);\n"
                    "SELECT 1 AS order_id, CURRENT_DATE AS order_date"
                ),
                "sql/models/marts/weekly/orders_weekly.sql": model_header(
                    key="model_schema", value="order_shape"
                ),
            },
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_valid_named_declaration_layout_when_compiling_then_project_is_accepted(
    test_case: NamedDeclarationAcceptedTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, {"sqlbuild_project.toml": _PROJECT_FILE} | test_case.files)

    compiled: CompiledProjectView = compile_and_assemble(project_dir=tmp_path)

    assert compiled.scope_index.completeness.placement is True
    assert (
        tuple(diagnostic.code.value for diagnostic in compiled.scope_index.diagnostics)
        == test_case.expected_diagnostic_codes
    )


@pytest.mark.parametrize(
    "test_case",
    (
        NamedDeclarationErrorTestCase(
            "audit file directly under sql/audits/",
            {"sql/models/orders.sql": _ORDERS, "sql/audits/check.sql": _GENERIC_AUDIT},
            ("Unsupported entries in sql/audits/", "sql/audits/generic/ or sql/audits/singular/"),
        ),
        NamedDeclarationErrorTestCase(
            "audit folder other than generic or singular",
            {"sql/models/orders.sql": _ORDERS, "sql/audits/checks/check.sql": _GENERIC_AUDIT},
            ("Unsupported entries in sql/audits/: sql/audits/checks",),
        ),
        NamedDeclarationErrorTestCase(
            "folder-only singular audit role",
            {
                "sql/models/marts/orders.sql": _ORDERS,
                "sql/models/marts/_sqlbuild/_audits/singular/check.sql": _GENERIC_AUDIT,
            },
            ("sql/models/marts/_sqlbuild/_audits/singular/ is invalid",),
        ),
        NamedDeclarationErrorTestCase(
            "_sqlbuild directly under a resource tree root",
            {
                "sql/models/orders.sql": model_header(key="audits", value="[order_check]"),
                "sql/models/_sqlbuild/audits/generic/order_check.sql": _GENERIC_AUDIT,
            },
            ("Grouped declaration root sql/models/_sqlbuild/ must be below a concrete owner",),
        ),
        NamedDeclarationErrorTestCase(
            "unsupported hook language folder",
            {
                "sql/models/marts/orders.sql": _ORDERS,
                "sql/models/marts/_sqlbuild/hooks/shell/touch.sql": _SQL_HOOK,
            },
            ("sql/hooks/ accepts only python/nodes/, sql/",),
        ),
        NamedDeclarationErrorTestCase(
            "scoped audit uses a constant that is not visible from the audit's folder",
            {
                "sql/models/staging/_sqlbuild/_constants/order_limit.sql": (
                    "CONSTANT (name order_limit, value 10);"
                ),
                "sql/models/staging/orders_stage.sql": (
                    'MODEL (description "Test model orders_stage.");\nSELECT @const("order_limit") AS order_id'
                ),
                "sql/models/marts/_sqlbuild/_audits/generic/order_check.sql": (
                    'AUDIT ();\nSELECT * FROM __ref("@model") '
                    'WHERE order_id > @const("order_limit")'
                ),
                "sql/models/marts/orders.sql": model_header(key="audits", value="[order_check]"),
            },
            ("order_limit", "known but inaccessible"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_named_declaration_layout_when_compiling_then_error_names_the_fix(
    test_case: NamedDeclarationErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, {"sqlbuild_project.toml": _PROJECT_FILE} | test_case.files)

    with pytest.raises(ValueError) as error:
        compile_and_assemble(project_dir=tmp_path)

    rendered: str = (
        f"[{getattr(error.value, 'code', '')}] {error.value} {getattr(error.value, 'help', '')}"
    )
    for fragment in test_case.expected_error_fragments:
        assert fragment in rendered


@pytest.mark.parametrize(
    "test_case",
    (
        NamedDeclarationErrorTestCase(
            "singular audit over one model",
            singular_audit_files(
                base=_SINGULAR_BASE_FILES,
                sql='SELECT * FROM __ref("orders") WHERE order_id IS NULL',
            ),
            ("[P004]", "checks only model 'orders'", "@relation"),
        ),
        NamedDeclarationErrorTestCase(
            "singular audit self-joining one model",
            singular_audit_files(
                base=_SINGULAR_BASE_FILES,
                sql='SELECT * FROM __ref("orders") a JOIN __ref("orders") b USING (order_id)',
            ),
            ("[P004]", "checks only model 'orders'"),
        ),
        NamedDeclarationErrorTestCase(
            "singular audit over a source only",
            singular_audit_files(
                base=_SINGULAR_BASE_FILES, sql='SELECT * FROM __source("raw_orders")'
            ),
            ("[P004]", "checks only sources or seeds"),
        ),
        NamedDeclarationErrorTestCase(
            "singular audit over a source and a seed",
            singular_audit_files(
                base=_SINGULAR_BASE_FILES,
                sql='SELECT * FROM __source("raw_orders") JOIN __seed("order_codes") USING (order_id)',
            ),
            ("[P004]", "checks only sources or seeds"),
        ),
        NamedDeclarationErrorTestCase(
            "singular audit over hard-coded relations",
            singular_audit_files(
                base=_SINGULAR_BASE_FILES, sql="SELECT * FROM information_schema.tables"
            ),
            ("[P004]", "references no SQLBuild resource"),
        ),
        NamedDeclarationErrorTestCase(
            "top-level generic audit used under one folder",
            {
                "sql/audits/generic/order_check.sql": _GENERIC_AUDIT,
                "sql/models/marts/orders.sql": model_header(key="audits", value="[order_check]"),
            },
            ("[S024]", "Move it to 'sql/models/marts/_sqlbuild/_audits/generic/'"),
        ),
        NamedDeclarationErrorTestCase(
            "top-level generic audit used across sibling sub-folders",
            {
                "sql/audits/generic/order_check.sql": _GENERIC_AUDIT,
                "sql/models/marts/daily/orders.sql": model_header(
                    key="audits", value="[order_check]"
                ),
                "sql/models/marts/weekly/orders_weekly.sql": model_header(
                    key="audits", value="[order_check]"
                ),
            },
            ("Move it to 'sql/models/marts/_sqlbuild/audits/generic/'",),
        ),
        NamedDeclarationErrorTestCase(
            "inherited generic audit used only by its own folder",
            {
                "sql/models/marts/_sqlbuild/audits/generic/order_check.sql": _GENERIC_AUDIT,
                "sql/models/marts/orders.sql": model_header(key="audits", value="[order_check]"),
            },
            ("Move it to 'sql/models/marts/_sqlbuild/_audits/generic/'",),
        ),
        NamedDeclarationErrorTestCase(
            "top-level singular audit referencing one folder",
            {
                "sql/models/marts/orders.sql": _ORDERS,
                "sql/models/marts/customers.sql": _CUSTOMERS,
                "sql/audits/singular/check.sql": (
                    'AUDIT ();\nSELECT * FROM __ref("orders") JOIN __ref("customers") '
                    "USING (order_id)"
                ),
            },
            (
                "References: model:customers, model:orders",
                "sql/models/marts/_sqlbuild/audits/singular/",
            ),
        ),
        NamedDeclarationErrorTestCase(
            "top-level generic audit used only through an audit factory",
            {
                "sql/audits/generic/order_check.sql": _GENERIC_AUDIT,
                "sql/models/marts/orders.sql": model_header(
                    key="audit_factories", value="[order_quality]"
                ),
                "python/nodes/factories/quality.py": (
                    "from sqlbuild.audits import AuditCase, audit_factory\n\n\n"
                    "@audit_factory\ndef order_quality():\n"
                    '    return [AuditCase(name="order_ok", definition="order_check")]\n'
                ),
            },
            ("Move it to 'sql/models/marts/_sqlbuild/_audits/generic/'",),
        ),
        NamedDeclarationErrorTestCase(
            "top-level schema used under one folder",
            {
                "sql/schemas/order_shape.sql": _SCHEMA,
                "sql/models/marts/orders.sql": model_header(
                    key="model_schema", value="order_shape"
                ),
            },
            ("Move it to 'sql/models/marts/_sqlbuild/_schemas/'",),
        ),
        NamedDeclarationErrorTestCase(
            "top-level SQL hook used under one folder",
            {
                "sql/hooks/sql/touch.sql": _SQL_HOOK,
                "sql/models/marts/orders.sql": model_header(
                    key="pre_hooks", value='[sql("touch")]'
                ),
            },
            ("Move it to 'sql/models/marts/_sqlbuild/_hooks/sql/'",),
        ),
        NamedDeclarationErrorTestCase(
            "top-level Python hook used under one folder",
            {
                "sql/hooks/python/lifecycle.py": _PYTHON_HOOK,
                "sql/models/marts/orders.sql": model_header(
                    key="pre_hooks", value='[python("mark")]'
                ),
            },
            ("Move it to 'sql/models/marts/_sqlbuild/_hooks/python/'",),
        ),
        NamedDeclarationErrorTestCase(
            "scoped generic audit used from a sibling folder",
            {
                "sql/models/marts/_sqlbuild/audits/generic/order_check.sql": _GENERIC_AUDIT,
                "sql/models/staging/orders.sql": model_header(key="audits", value="[order_check]"),
            },
            (
                "[S006]",
                "Generic audit 'order_check' is not visible from 'sql/models/staging/orders.sql'",
            ),
        ),
        NamedDeclarationErrorTestCase(
            "every consumer of an invisible scoped audit is reported",
            {
                "sql/models/marts/_sqlbuild/audits/generic/order_check.sql": _GENERIC_AUDIT,
                "sql/models/staging/orders.sql": model_header(key="audits", value="[order_check]"),
                "sql/models/staging/customers.sql": model_header(
                    key="audits", value="[order_check]"
                ),
            },
            (
                "[S006] Generic audit 'order_check' is not visible from "
                "'sql/models/staging/customers.sql'",
                "[S006] Generic audit 'order_check' is not visible from 'sql/models/staging/orders.sql'",
            ),
        ),
        NamedDeclarationErrorTestCase(
            "scoped generic audit used from a parent folder",
            {
                "sql/models/marts/daily/_sqlbuild/audits/generic/order_check.sql": _GENERIC_AUDIT,
                "sql/models/marts/orders.sql": model_header(key="audits", value="[order_check]"),
            },
            ("Generic audit 'order_check' is not visible from 'sql/models/marts/orders.sql'",),
        ),
        NamedDeclarationErrorTestCase(
            "scoped schema bound from a sibling folder",
            {
                "sql/models/marts/_sqlbuild/_schemas/order_shape.sql": _SCHEMA,
                "sql/models/staging/orders.sql": model_header(
                    key="model_schema", value="order_shape"
                ),
            },
            ("Schema 'order_shape' is not visible from 'sql/models/staging/orders.sql'",),
        ),
        NamedDeclarationErrorTestCase(
            "scoped schema bound from a parent folder",
            {
                "sql/models/marts/daily/_sqlbuild/schemas/order_shape.sql": _SCHEMA,
                "sql/models/marts/orders.sql": model_header(
                    key="model_schema", value="order_shape"
                ),
            },
            ("Schema 'order_shape' is not visible from 'sql/models/marts/orders.sql'",),
        ),
        NamedDeclarationErrorTestCase(
            "scoped SQL hook called from a sibling folder",
            {
                "sql/models/marts/_sqlbuild/_hooks/sql/touch.sql": _SQL_HOOK,
                "sql/models/staging/orders.sql": model_header(
                    key="pre_hooks", value='[sql("touch")]'
                ),
            },
            ("SQL hook 'touch' is not visible from 'sql/models/staging/orders.sql'",),
        ),
        NamedDeclarationErrorTestCase(
            "scoped SQL hook called from a parent folder",
            {
                "sql/models/marts/daily/_sqlbuild/hooks/sql/touch.sql": _SQL_HOOK,
                "sql/models/marts/orders.sql": model_header(
                    key="pre_hooks", value='[sql("touch")]'
                ),
            },
            ("SQL hook 'touch' is not visible from 'sql/models/marts/orders.sql'",),
        ),
        NamedDeclarationErrorTestCase(
            "scoped Python hook called from a sibling folder",
            {
                "sql/models/marts/_sqlbuild/_hooks/python/lifecycle.py": _PYTHON_HOOK,
                "sql/models/staging/orders.sql": model_header(
                    key="pre_hooks", value='[python("mark")]'
                ),
            },
            ("Python hook 'mark' is not visible from 'sql/models/staging/orders.sql'",),
        ),
        NamedDeclarationErrorTestCase(
            "scoped Python hook called from a parent folder",
            {
                "sql/models/marts/daily/_sqlbuild/hooks/python/lifecycle.py": _PYTHON_HOOK,
                "sql/models/marts/orders.sql": model_header(
                    key="pre_hooks", value='[python("mark")]'
                ),
            },
            ("Python hook 'mark' is not visible from 'sql/models/marts/orders.sql'",),
        ),
        NamedDeclarationErrorTestCase(
            "child schema extends a parent from a sibling folder",
            {
                "sql/models/staging/_sqlbuild/schemas/order_shape.sql": _SCHEMA,
                "sql/models/marts/_sqlbuild/_schemas/daily_shape.sql": (
                    "SCHEMA (name daily_shape, extends order_shape, "
                    "columns (order_date (type DATE)));"
                ),
                "sql/models/marts/orders.sql": model_header(
                    key="model_schema", value="daily_shape"
                ),
                "sql/models/staging/orders_stage.sql": model_header(
                    key="model_schema", value="order_shape"
                ),
            },
            ("Schema 'order_shape' is not visible from",),
        ),
        NamedDeclarationErrorTestCase(
            "scoped schema uses an enum type that is not visible from the schema's folder",
            {
                "sql/models/staging/_sqlbuild/_enums/order_status.sql": (
                    "ENUM (name order_status, members [OPEN, CLOSED]);"
                ),
                "sql/models/staging/orders_stage.sql": (
                    'MODEL (description "Test model orders_stage.");\nSELECT @enum("order_status").OPEN AS status'
                ),
                "sql/models/marts/_sqlbuild/_schemas/order_shape.sql": (
                    "SCHEMA (name order_shape, columns (status (type order_status)));"
                ),
                "sql/models/marts/orders.sql": (
                    "MODEL (model_schema order_shape);\nSELECT 'OPEN' AS status"
                ),
            },
            ("[S006]", "uses enum 'order_status', which is not visible from the schema's location"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_named_declaration_use_when_compiling_then_diagnostics_name_the_fix(
    test_case: NamedDeclarationErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, {"sqlbuild_project.toml": _PROJECT_FILE} | test_case.files)

    rendered: str = render_compile_diagnostics(project=compile_and_assemble(project_dir=tmp_path))

    for fragment in test_case.expected_error_fragments:
        assert fragment in rendered


@pytest.mark.parametrize(
    "test_case",
    (
        NamedDeclarationWarningTestCase(
            "advisory placement keeps a misplaced scoped schema compiling",
            {
                "sql/schemas/order_shape.sql": _SCHEMA,
                "sql/models/marts/orders.sql": model_header(
                    key="model_schema", value="order_shape"
                ),
            },
            "S024",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_advisory_placement_when_compiling_misplaced_declaration_then_warns(
    test_case: NamedDeclarationWarningTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, {"sqlbuild_project.toml": _ADVISORY_PROJECT_FILE} | test_case.files)

    compiled: CompiledProjectView = compile_and_assemble(project_dir=tmp_path)

    assert tuple(
        (diagnostic.code.value, diagnostic.severity)
        for diagnostic in compiled.scope_index.diagnostics
    ) == ((test_case.expected_code, DiagnosticSeverity.WARNING),)
