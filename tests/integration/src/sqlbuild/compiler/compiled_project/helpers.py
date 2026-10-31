"""Helpers for native compiled project integration tests."""

from __future__ import annotations

import json
import shutil
from collections import Counter
from collections.abc import Callable, Iterator
from pathlib import Path
from types import ModuleType
from typing import TYPE_CHECKING, Any
from uuid import UUID

import pytest

import sqlbuild._native as _native
from sqlbuild._native import (
    CollectionRendering,
    CompileAdapterContext,
    CompileModelConfig,
    CompileModelInput,
    CompileProjectInputs,
    CompilerDiagnostic,
    DiscoveredProjectInputs,
    DiscoveredSqlModelFile,
    LocalConfig,
    MeasurementThresholdBound,
    MeasurementThresholds,
    ProjectConfig,
    SourceColumnEntry,
    SqlLexicalSyntax,
    assemble_project,
    build_compile_inputs,
    build_compiled_project,
    discover_project_inputs,
    extract_sql_references,
)
from sqlbuild.adapter.contract.models import ExpressionInferenceProfile
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import (
    CompiledAuditView,
    CompiledFunctionView,
    CompiledModelView,
    CompiledProjectView,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView

METADATA: dict[str, Any] = {
    "location_id": str(UUID(int=1)),
    "target": "development",
    "generation": 1,
    "git_revision": None,
    "git_dirty": False,
    "git_author": None,
    "image_id": None,
    "environment_hash": "test-environment",
    "sqb_version": "not-the-installed-version",
    "nodes": {},
    "documents": {},
}

TYPE_PROOF_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQBRCONTRACT105"]\n\n'
        '[defaults]\ncontract = "enforced"\n'
    ),
    "models/stg_orders.sql": (
        "MODEL (description 'Staged orders.',\n"
        "  columns (order_id (type INTEGER), amount (type INTEGER)),\n"
        ");\n\n"
        "SELECT CAST(1 AS INTEGER) AS order_id, CAST(5 AS INTEGER) AS amount\n"
    ),
    "models/orders.sql": (
        "MODEL (description 'Typed orders.',\n"
        "  columns (order_id (type INTEGER), amount (type BIGINT)),\n"
        ");\n\n"
        'SELECT order_id, amount\nFROM __ref("stg_orders")\n'
    ),
}


_FACTS_CONSUMERS: tuple[ModuleType, ...] = (_native,)


def forbid_hand_built_projects(*, monkeypatch: pytest.MonkeyPatch) -> None:
    """Fail if a consumer records a project from `CompiledProject` instead of the compile's."""

    def retained_only(project: CompiledProjectView) -> _native.NativeCompiledProject:
        retained: _native.NativeCompiledProject | None = project.native_project
        assert retained is not None, f"rules rebuilt project facts from {type(project).__name__}"
        return retained

    for module in _FACTS_CONSUMERS:
        monkeypatch.setattr(module, "compiled_project_facts", retained_only)


LEGACY_PROJECT_TYPES: tuple[str, ...] = (
    "CompileProjectInputs",
    "CompileModelInput",
    "CompiledProject",
    "CompiledModel",
)


def count_legacy_constructions(*, monkeypatch: pytest.MonkeyPatch) -> Counter[str]:
    """Count every construction of the Python project representation, `replace` included."""

    counts: Counter[str] = Counter()
    for name in LEGACY_PROJECT_TYPES:
        model_type: type = getattr(_native, name)
        monkeypatch.setattr(
            model_type, "__init__", _counted_init(counts=counts, name=name, model_type=model_type)
        )
    return counts


def _counted_init(*, counts: Counter[str], name: str, model_type: type) -> object:
    original: Callable[..., None] = model_type.__init__

    def counted(self: object, *args: object, **kwargs: object) -> None:
        counts[name] += 1
        original(self, *args, **kwargs)

    return counted


PARITY_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n\n[vars]\nregion = "eu"\n\n'
        '[defaults]\nmaterialized = "table"\n\n[path_defaults.staging]\n'
        'materialized = "view"\ntags = ["staged"]\n'
    ),
    "seeds/payment_methods.csv": "method\ncard\n",
    "seeds/payment_methods.yml": (
        "seeds:\n  - name: payment_methods\n    description: Payment methods.\n"
        "    columns:\n      - name: method\n        type: VARCHAR\n"
    ),
    "models/staging/stg_orders.sql": (
        "MODEL (description 'Staged orders.', tags [daily]);\n"
        "SELECT 1 AS order_id, '@@region' AS region -- note\n"
    ),
    "models/staging/stg_payments.sql": (
        'MODEL (tags [payments]);\nSELECT method FROM __seed("payment_methods")\n'
    ),
    "models/marts/orders.sql": (
        "MODEL (description 'Orders.', materialized incremental, incremental_strategy append,"
        " cursor order_id, cursor_type integer);\n"
        'SELECT order_id FROM __ref("stg_orders")\n'
        "WHERE order_id >= __cursor_start()\n"
    ),
    "models/marts/opted_out.sql": ("MODEL (sql_analysis false);\nSELECT 1 AS order_id\n"),
    "models/marts/described_columns.sql": (
        "MODEL (\n  columns (order_id (type INTEGER, nullable false,"
        ' audits [not_null, unique]), note (description "Free text.")),\n'
        ");\n"
        "SELECT 1 AS order_id, 'x' AS note\n"
    ),
    "enums/status.sql": "ENUM (name order_status, members [PLACED, SHIPPED]);\n",
    "constants/limits.sql": (
        "CONSTANT (name order_cap, value 3);\nCONSTANT (name held_codes, value [30, 31]);\n"
    ),
    "models/marts/statuses.sql": (
        "MODEL (columns (status (type VARCHAR)));\n"
        'SELECT @enum("order_status").PLACED AS status, @const("order_cap") AS cap\n'
        'WHERE 30 NOT IN @const("held_codes")\n'
    ),
    "macros/python/money.py": 'def cents(value: str) -> str:\n    return f"{value} * 100"\n',
    "models/marts/priced.sql": (
        "MODEL (description 'Priced orders.');\n"
        "SELECT @py.cents('3') AS cents, order_id FROM __ref(\"stg_orders\")\n"
    ),
    "models/marts/hooked.sql": (
        "MODEL (\n  description 'Hooked orders.',\n"
        "  pre_hooks [inline_sql('SELECT @@region AS region')],\n"
        "  post_hooks [inline_sql('SELECT @py.cents(2) AS cents')],\n);\n"
        "SELECT 1 AS order_id\n"
    ),
    "models/marts/migrated.sql": (
        "MODEL (materialized incremental, incremental_strategy append,"
        " columns (order_id (migrate_from id)));\nSELECT 1 AS order_id\n"
    ),
}


HEADER_CONTRACT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
    "enums/status.sql": "ENUM (name order_status, members [PLACED, SHIPPED]);\n",
    "audits/generic/expression_is_true.sql": (
        'AUDIT ();\nSELECT * FROM __ref("@model") WHERE NOT (@expression)\n'
    ),
    "python/quality.py": (
        "from sqlbuild.audits import AuditCase, audit_factory\n\n\n"
        "@audit_factory\n"
        "def order_checks():\n"
        "    return [\n"
        '        AuditCase(name="positive_total", definition="expression_is_true",\n'
        '                  arguments={"expression": "total > 0"}),\n'
        "    ]\n"
    ),
    "models/order_amounts.sql": (
        "MODEL (columns (customer_id (type INTEGER)));\n"
        "SELECT 1 AS customer_id, 'books' AS category, 2 AS amount\n"
    ),
    "models/category_amounts.sql": (
        "MODEL (\n  columns (customer_id (type INTEGER)),\n"
        "  dynamic_columns (category_amounts (pivot_column category, value_column amount,"
        ' aggregate max, type INTEGER, name_pattern "^[a-z]+$")),\n);\n'
        'SELECT * FROM __ref("order_amounts")\n'
        "PIVOT(MAX(amount) FOR category IN (ANY ORDER BY category))\n"
    ),
    "models/order_statuses.sql": (
        "MODEL (\n  contract enforced,\n"
        "  columns (status (type order_status, audits [not_null]), total (type INTEGER)),\n"
        "  audit_factories [order_checks],\n);\n"
        'SELECT @enum("order_status").PLACED AS status, 3 AS total\n'
    ),
}


OPT_OUT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n\n[settings]\nrequire_sql_analysis = true\n\n'
        "[path_defaults.legacy]\nsql_analysis = false\n"
    ),
    "models/header_opt_out.sql": "MODEL (\n  sql_analysis false\n);\nSELECT 1 AS order_id\n",
    "models/legacy/path_opt_out.sql": "MODEL ();\nSELECT 1 AS order_id\n",
    "models/unparsed.sql": "MODEL (sql_analysis false);\nSELECT FROM WHERE (\n",
    "models/invalid_call.sql": (
        "MODEL ();\nSELECT * FROM __ref(order_name)\nCROSS JOIN __table_fn('customer_orders')\n"
    ),
}


MODEL_SCHEMA_FILES: dict[str, str] = {
    "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
    "enums/status.sql": "ENUM (name order_status, members [PLACED, SHIPPED]);\n",
    "schemas/orders.sql": (
        "SCHEMA (\n  name order_shape,\n  description 'Order shape.',\n"
        "  columns (order_id (type INTEGER, nullable false), status (type order_status)),\n);\n"
    ),
    "models/marts/_sqlbuild/schemas/local.sql": (
        "SCHEMA (\n  name marts_shape,\n  columns (order_id (type INTEGER)),\n);\n"
    ),
    "models/orders.sql": (
        "MODEL (\n  contract enforced,\n  model_schema order_shape,\n"
        "  columns (order_id (audits [unique]), note (type VARCHAR)),\n);\n"
        "SELECT 1 AS order_id, 'PLACED' AS status, 'x' AS note\n"
    ),
    "models/staging/hidden.sql": (
        "MODEL (\n  description 'Out of reach.',\n  model_schema marts_shape,\n);\n"
        "SELECT 1 AS order_id\n"
    ),
}


PYTHON_HOOK_FILES: dict[str, str] = {
    "hooks/python/notifications.py": (
        "from sqlbuild.hooks import hook\n\n\n"
        "@hook\ndef notify(ctx, channel, *, level='info'):\n"
        '    """Notify a channel."""\n    return None\n\n\n'
        "@hook\ndef positional(warehouse, /, ctx):\n"
        '    """Take a positional-only warehouse."""\n    return None\n\n\n'
        "@hook\ndef flexible(ctx, **options):\n"
        '    """Accept any configured argument."""\n    return None\n'
    ),
}


PYTHON_HOOK_PARITY_FILES: dict[str, str] = PYTHON_HOOK_FILES | {
    "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
    "models/marts/_sqlbuild/hooks/python/local.py": (
        "from sqlbuild.hooks import hook\n\n\n"
        "@hook\ndef refresh(ctx):\n"
        '    """Refresh marts."""\n    return None\n'
    ),
    "models/orders.sql": (
        'MODEL (\n  pre_hooks [python("notify", channel: "ops")],\n'
        '  post_hooks [python("flexible", any: 1), python("notify", channel: "c", level: "x")],\n'
        ");\n"
        "SELECT 1 AS order_id\n"
    ),
    "models/marts/summary.sql": (
        'MODEL (post_hooks [python("refresh"), python("flexible")]);\nSELECT 1 AS order_id\n'
    ),
    "models/staging/hidden.sql": (
        'MODEL (post_hooks [python("refresh")]);\nSELECT 1 AS order_id\n'
    ),
}


SQL_HOOK_FILES: dict[str, str] = {
    "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n\n[vars]\nregion = "eu"\n',
    "enums/status.sql": "ENUM (name order_status, members [PLACED, SHIPPED]);\n",
    "constants/limits.sql": "CONSTANT (name order_cap, value 3);\n",
    "macros/python/money.py": 'def cents(value: str) -> str:\n    return f"{value} * 100"\n',
    "hooks/sql/grant_read.sql": (
        "HOOK (description 'Grant read access.');\n\n"
        "GRANT SELECT ON @@CTX:destination.qualified TO @'role'\n"
    ),
    "hooks/sql/log_load.sql": (
        "HOOK (description 'Log a load.');\n\n"
        'INSERT INTO @target SELECT @enum("order_status").PLACED, @const("order_cap"),'
        " @py.cents(@amount), '@@region' FROM __ref(\"stg_orders\")\n"
    ),
    "models/staging/stg_orders.sql": "MODEL ();\nSELECT 1 AS order_id\n",
    "models/marts/loaded.sql": (
        "MODEL (\n  schema analytics,\n"
        '  pre_hooks [sql("grant_read", role: ["reader", "writer"])],\n'
        '  post_hooks [sql("log_load", target: "audit.loads", amount: 5),'
        ' inline_sql(\'SELECT @const("order_cap") FROM __ref("stg_orders")\')],\n);\n'
        'SELECT order_id FROM __ref("stg_orders")\n'
    ),
}

MACRO_FILES: dict[str, str] = {
    "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n\n[vars]\nregion = "eu"\n',
    "macros/python/amounts.py": (
        '"""Amount macros."""\n\n\n'
        'def cents(value):\n    """Cents."""\n    return f"{value} * 100"\n\n\n'
        'def word():\n    """A word."""\n    return "x"\n'
    ),
}


def build_inputs_with_native_loop(
    *, project_dir: Path, defer_validation: bool = True
) -> CompileInputsView:
    """Build compile inputs through the native model loop."""

    return build_compile_inputs(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter_context=CompileAdapterContext(
            value_renderer=DuckDbAdapter(),
            collection_rendering=CollectionRendering.VALUE_LIST,
            python_functions_inherit_default_namespace=True,
            sql_lexical_syntax=DuckDbAdapter.sql_lexical_syntax,
        ),
        defer_model_sql_validation=defer_validation,
        no_cache=True,
    )


_SUMMARY_FIELDS: tuple[str, ...] = (
    "config",
    "query_sql",
    "macro_source_sql",
    "references",
    "schema_entry",
    "sql_validation_enabled",
    "rejected_sql_analysis_opt_out",
    "enum_declarations",
    "constant_declarations",
    "enum_columns",
    "macro_deps",
    "macro_usages",
    "declaration_usages",
    "sql_expansion",
    "lint_expansion",
)


def inputs_summary(*, inputs: CompileInputsView, project_dir: Path) -> dict[str, object]:
    """Each model input's fields and the diagnostics, as text with the project path elided."""

    return {
        "models": {
            model_input.model_file.relative_path.as_posix(): _model_input_fields(
                model_input=model_input, project_dir=project_dir
            )
            for model_input in inputs.model_inputs
        },
        "diagnostics": [
            _elided(value=item, project_dir=project_dir) for item in inputs.diagnostics
        ],
    }


def _model_input_fields(*, model_input: object, project_dir: Path) -> dict[str, str]:
    return {
        field_name: _elided(value=getattr(model_input, field_name), project_dir=project_dir)
        for field_name in _SUMMARY_FIELDS
    }


def _elided(*, value: object, project_dir: Path) -> str:
    return repr(value).replace(str(project_dir), "<project>")


def expected_summary(name: str) -> dict[str, object]:
    """The recorded summary of the attachment's compile inputs for one parity project."""

    return json.loads((EXPECTED_SUMMARIES / f"{name}.json").read_text(encoding="utf-8"))


EXPECTED_SUMMARIES: Path = Path(__file__).parent / "expected"


WAFFLE_SHOP_NATIVE_MODELS: int = 13


def copy_project(*, fixtures: tuple[str, ...], files: dict[str, str], project_dir: Path) -> Path:
    """Copy each fixture into `project_dir`, then write `files` over them."""

    project_dir.mkdir(parents=True)
    for fixture in fixtures:
        _ = shutil.copytree(fixture, project_dir, dirs_exist_ok=True)
    write_files(project_dir=project_dir, files=files)
    return project_dir


SOURCE_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n\n[vars]\nregion = "eu"\n\n'
        '[defaults]\ncontract = "enforced"\n'
    ),
    "enums/status.sql": "ENUM (name order_status, members [PLACED, SHIPPED]);\n",
    "constants/limits.sql": "CONSTANT (name order_cap, value 3);\n",
    "macros/python/money.py": 'def cents(value: str) -> str:\n    return f"{value} * 100"\n',
}
_SOURCE_CONNECTION: dict[str, object] = {"database": "warehouse", "schema": "main"}


def compile_source_project(
    *, project_dir: Path, source_yml: str, monkeypatch: pytest.MonkeyPatch
) -> tuple[CompiledProjectView, CompileInputsView]:
    """Compile a project declaring `source_yml`; return the project and its compile inputs."""

    write_files(
        project_dir=project_dir, files={**SOURCE_PROJECT_FILES, "sources/raw.yml": source_yml}
    )
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter=DuckDbAdapter(),
        resolved_connection=_SOURCE_CONNECTION,
    )
    inputs: CompileInputsView = build_inputs_with_native_loop(project_dir=project_dir)
    return project, inputs


def column_metadata(column: SourceColumnEntry) -> tuple[str | None, tuple[dict[str, object], ...]]:
    """A source column's description and its audits' arguments."""

    return column.description, tuple(audit.arguments for audit in column.audits)


FUNCTION_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n\n[vars]\nregion = "eu"\n'
        'amount_type = "DECIMAL(10, 2)"\n\n[defaults]\nfunction_schema = "udfs_${region}"\n'
    ),
    "enums/status.sql": "ENUM (name order_status, members [PLACED, SHIPPED]);\n",
    "models/orders.sql": "MODEL (description 'Orders.', schema analytics);\nSELECT 1 AS order_id\n",
    "functions/sql/order_total.sql": (
        "FUNCTION (description 'Order total.', tags [finance],"
        " arguments (amount ${amount_type}), returns ${amount_type});\nSELECT amount * 2\n"
    ),
    "functions/sql/open_orders.sql": (
        "FUNCTION (description 'Open orders.', schema reporting,"
        " returns table (order_id INTEGER, status VARCHAR));\n"
        'SELECT order_id, @enum("order_status").PLACED AS status FROM __ref("orders")\n'
    ),
    "functions/python/score.py": (
        "from sqlbuild.functions import udf\n\n"
        "@udf(arguments={'value': 'INTEGER'}, returns='INTEGER', runtime_version='3.11',"
        " packages=['numpy'], tags=['ml'])\n"
        "def main(value):\n    '''Score.'''\n    return value\n"
    ),
}


def compile_function_project(
    *, project_dir: Path, files: dict[str, str], monkeypatch: pytest.MonkeyPatch
) -> tuple[CompiledProjectView, CompileInputsView]:
    """Compile a project of `files`; return the project and its compile inputs."""

    write_files(project_dir=project_dir, files=files)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter=DuckDbAdapter(),
        resolved_connection=_SOURCE_CONNECTION,
    )
    inputs: CompileInputsView = build_inputs_with_native_loop(project_dir=project_dir)
    return project, inputs


def function_summary(function: CompiledFunctionView) -> tuple[object, ...]:
    """A compiled function's planner-facing facts."""

    return (
        type(function).__name__,
        str(function.key.resource_type),
        tuple(dep.name for dep in function.deps),
        tuple((argument.name, argument.type) for argument in function.arguments),
        function.returns,
        tuple((column.name, column.type) for column in function.return_columns),
        tuple(reference.ref_name for reference in function.references),
        function.language.value,
        function.destination.qualified_name,
        function.fingerprint_destination.qualified_name,
        (function.runtime_version, function.packages, function.tags, function.description),
    )


AUDIT_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n\n[settings]\ndefault_audit_severity = "warn"\n'
    ),
    "models/orders.sql": (
        "MODEL (description 'Orders.', columns (order_id (type INTEGER,"
        " audits [not_null, at_least(minimum 1, severity error)])));\nSELECT 1 AS order_id\n"
    ),
    "models/customers.sql": "MODEL (description 'Customers.');\nSELECT 1 AS customer_id\n",
    "audits/generic/at_least.sql": (
        "AUDIT (name at_least);\nSELECT * FROM @relation WHERE @column < @minimum\n"
    ),
    "audits/singular/orders_have_customers.sql": (
        "AUDIT (name orders_have_customers, run_scope final);\n"
        'SELECT * FROM __ref("orders") o LEFT JOIN __ref("customers") c'
        " ON o.order_id = c.customer_id WHERE c.customer_id IS NULL\n"
    ),
}


def audit_summary(audit: CompiledAuditView) -> tuple[object, ...]:
    """A compiled audit's planner-facing facts."""

    return (
        type(audit).__name__,
        audit.definition_name,
        tuple(dep.name for dep in audit.scope_deps),
        str(audit.severity),
        audit.run_scope,
        (audit.attached_target_name, audit.attached_column_name),
        audit.sql_body,
    )


def measurement_policy(audit: CompiledAuditView) -> tuple[object, ...]:
    """A measurement audit's bound operators and limits, sample count and severity."""

    thresholds: MeasurementThresholds | None = audit.thresholds
    assert thresholds is not None
    return (
        *_bound_parts(thresholds.warn),
        *_bound_parts(thresholds.error),
        audit.minimum_samples,
        str(audit.severity),
    )


def _bound_parts(bound: MeasurementThresholdBound | None) -> tuple[object, object]:
    return (getattr(getattr(bound, "operator", None), "value", None), getattr(bound, "limit", None))


RESOURCE_DIAGNOSTIC_CODES: frozenset[str] = frozenset({"P010", "P011", "C216"})
RESOURCE_PROJECT_FILE: str = 'name = "orders"\nadapter = "duckdb"\n'


def resource_diagnostics_of(
    *, project_dir: Path, files: dict[str, str], monkeypatch: pytest.MonkeyPatch
) -> tuple[tuple[object, ...], ...]:
    """Each P010, P011 and C216 diagnostic of compiling `files`, as code, message and location."""

    write_files(
        project_dir=project_dir, files={"sqlbuild_project.toml": RESOURCE_PROJECT_FILE, **files}
    )
    inputs: CompileInputsView = build_inputs_with_native_loop(project_dir=project_dir)
    resource_diagnostics: Iterator[CompilerDiagnostic] = filter(
        lambda diagnostic: diagnostic.code in RESOURCE_DIAGNOSTIC_CODES, inputs.diagnostics
    )
    return tuple(
        (
            diagnostic.code,
            diagnostic.message,
            str(diagnostic.path),
            diagnostic.line,
            diagnostic.column,
            getattr(diagnostic.location, "end_column", None),
        )
        for diagnostic in resource_diagnostics
    )


_CURSOR_MODEL: str = (
    "MODEL (\n"
    '  description "Daily revenue",\n'
    "  materialized incremental,\n"
    "  incremental_strategy delete_insert,\n"
    "  cursor revenue_date,\n"
    "  cursor_type timestamp,\n"
    "  cursor_grain day,\n"
    ");\n\n"
    "SELECT CAST('2026-04-01' AS DATE) AS revenue_date{read}\n"
)


def cursor_model_files(*, read: str = "") -> dict[str, str]:
    """An upstream payments model and an incremental cursor model reading `read`."""

    return {
        "models/payments.sql": 'MODEL (description "Payments");\nSELECT 1 AS id\n',
        "models/finance/daily_revenue.sql": _CURSOR_MODEL.format(read=read),
    }


_HAND_BUILT_SYNTAX: SqlLexicalSyntax = SqlLexicalSyntax()
_HAND_BUILT_CONFIG: ProjectConfig = ProjectConfig(name="orders", adapter="duckdb")


def _hand_built_model(name: str, sql: str) -> CompileModelInput:
    relative_path: Path = Path(f"models/{name}.sql")
    return CompileModelInput(
        model_file=DiscoveredSqlModelFile(
            file_path=Path("/repo") / relative_path,
            relative_path=relative_path,
            contents=f"MODEL (description 'Test model.');\n\n{sql}\n",
            header_values={},
            header_column_locations={},
            output_column_locations={},
            query_sql=sql,
        ),
        config=CompileModelConfig(values={}),
        query_sql=sql,
        references=extract_sql_references(sql=sql, syntax=_HAND_BUILT_SYNTAX),
        sql_validation_enabled=True,
    )


def hand_built_analysed_project(models: dict[str, str]) -> CompiledProjectView:
    """Assemble hand-built model inputs, which cross the explicit analysis boundary."""

    return assemble_project(
        inputs=CompileProjectInputs(
            project_config=_HAND_BUILT_CONFIG,
            local_config=LocalConfig(),
            sql_lexical_syntax=_HAND_BUILT_SYNTAX,
            discovered_inputs=DiscoveredProjectInputs(
                project_config=_HAND_BUILT_CONFIG, local_config=LocalConfig()
            ),
            model_inputs=tuple(map(_hand_built_model, models, models.values())),
        ),
        inference_profile=ExpressionInferenceProfile(sql_analysis_dialect="duckdb"),
    )


def native_analysed_project(*, project_dir: Path, models: dict[str, str]) -> CompiledProjectView:
    """Compile the same models from files through the native owner."""

    write_files(
        project_dir=project_dir,
        files={
            "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
            **{
                f"models/{name}.sql": f"MODEL (description 'Test model.');\n\n{sql}\n"
                for name, sql in models.items()
            },
        },
    )
    return build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter=DuckDbAdapter(),
        resolved_connection=_SOURCE_CONNECTION,
    )


def model_analysis_summary(project: CompiledProjectView) -> tuple[tuple[object, ...], ...]:
    """Each model's column names, lineage outputs, star flag and located binding diagnostics."""

    return tuple(map(_model_analysis, sorted(project.models, key=_model_name)))


def _model_name(model: CompiledModelView) -> str:
    return model.name


def _model_analysis(model: CompiledModelView) -> tuple[object, ...]:
    return (
        model.name,
        tuple(column.name for column in model.inferred_columns or ()),
        tuple(column.output_column for column in model.fast_lineage_columns or ()),
        model.fast_lineage_has_star,
        model.binding_validated,
        tuple(map(_located_binding, model.binding_diagnostics)),
    )


def _located_binding(diagnostic: CompilerDiagnostic) -> tuple[object, ...]:
    return (
        diagnostic.code,
        diagnostic.line,
        diagnostic.column,
        getattr(diagnostic.location, "end_column", None),
    )
