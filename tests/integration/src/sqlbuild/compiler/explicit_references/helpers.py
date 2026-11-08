"""Projects whose explicit-reference checks raise or report, and their full compile outcomes."""

from __future__ import annotations

from collections.abc import Callable
from operator import attrgetter
from pathlib import Path
from typing import Any, cast

from sqlbuild._native import (
    CollectionRendering,
    CompileAdapterContext,
    CompilerDiagnostic,
    LiteralSqlRelation,
    assemble_project,
    build_compile_inputs,
    discover_project_inputs,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView

_ADAPTER_CONTEXT: CompileAdapterContext = CompileAdapterContext(
    value_renderer=DuckDbAdapter(),
    collection_rendering=CollectionRendering.VALUE_LIST,
    python_functions_inherit_default_namespace=True,
    sql_lexical_syntax=DuckDbAdapter.sql_lexical_syntax,
    sql_analysis_dialect=DuckDbAdapter.sql_analysis_dialect_name,
)
_PROJECT: str = (
    'name = "orders_project"\nadapter = "duckdb"\n\n'
    "[settings]\nsql_analysis = false\nsql_validation = false\n"
)
_DISABLED: str = _PROJECT + "\n[references]\nenforce_explicit = false\n"
_OLD_NAMES_OFF: str = _PROJECT + "\n[migrations]\nold_name_views = false\n"
_HOOK_PATH: str = "models/marts/_sqlbuild/_hooks/python/lookups.py"
_SQL_HOOK_PATH: str = "models/marts/_sqlbuild/_hooks/sql/record_rows.sql"
_TASK_PATH: str = "python/tasks/export.py"
_CHECK_PATH: str = "python/checks/orders.py"
_ASSET_PATH: str = "python/assets/snapshot.py"
_LOADER_PATH: str = "python/loaders/raw.py"
_HOOKED: str = 'MODEL (description "Orders.", post_hooks [python("refresh_lookup")]);'
_NAMED_SQL_HOOK: str = 'MODEL (description "Orders.", post_hooks [sql("record_rows")]);'


def _model(sql: str, header: str = 'MODEL (description "Test model.");') -> str:
    return f"{header}\n{sql}\n"


def _hook(
    *, reads: str = "()", body: str = "    return None\n", name: str = "refresh_lookup"
) -> str:
    return (
        "from sqlbuild.hooks import hook\nfrom sqlbuild.refs import model, seed, source\n\n\n"
        f"@hook(reads={reads})\ndef {name}(ctx):\n{body}"
    )


def _task(body: str, depends_on: str = 'model("orders")') -> str:
    return (
        "from sqlbuild.refs import model, source\nfrom sqlbuild.tasks import task\n\n\n"
        f"@task(depends_on={depends_on})\ndef export_orders(ctx):\n{body}"
    )


def _check(body: str) -> str:
    return (
        "from sqlbuild.checks import check\nfrom sqlbuild.refs import model\n\n\n"
        '@check(depends_on=model("orders"))\ndef orders_present(ctx):\n'
        f"{body}    return ctx.pass_()\n"
    )


def _asset(body: str) -> str:
    return (
        "from sqlbuild.assets import asset\nfrom sqlbuild.refs import model\n\n\n"
        '@asset(depends_on=model("orders"))\ndef orders_snapshot(context):\n'
        f'    """Test asset."""\n{body}    return None\n'
    )


def _loaders(*, depends_on: str, body: str) -> str:
    return (
        "from sqlbuild.loaders import loader\n\n\n"
        "@loader\ndef raw_regions(ctx):\n    '''Test loader raw_regions.'''\n"
        "    return [{'id': 1}]\n\n\n"
        f"@loader(depends_on=[{depends_on}])\ndef raw_customers(ctx):\n{body}"
        "    return [{'id': 1}]\n"
    )


_MODELS: dict[str, str] = {
    "models/marts/customers.sql": _model("SELECT 1 AS customer_id"),
    "models/marts/orders.sql": _model("SELECT 1 AS order_id", header=_HOOKED),
}
_SEED: dict[str, str] = {
    "seeds/country_codes.yml": (
        "seeds:\n  - name: country_codes\n    description: Country codes.\n"
        "    columns:\n      - name: code\n        type: VARCHAR\n"
    ),
    "seeds/country_codes.csv": "code\nGB\n",
}
_MANAGED: dict[str, str] = {
    "sources/raw.yml": (
        "sources:\n"
        "  - name: raw_regions\n    description: Raw regions.\n    managed: true\n"
        "    write_strategy: table\n    columns:\n      - name: id\n        type: INTEGER\n"
        "  - name: raw_customers\n    description: Raw customers.\n    managed: true\n"
        "    write_strategy: table\n    columns:\n      - name: id\n        type: INTEGER\n"
    ),
}
_EXTERNAL: dict[str, str] = {
    "sources/landing.yml": (
        "sources:\n  - name: landing_orders\n    description: Landing orders.\n"
        "    schema: landing\n    table: orders_feed\n"
        "  - name: computed\n    description: Computed.\n    expression: SELECT 1 AS id\n"
    ),
}
_GATED_AUDIT: str = "AUDIT ();\nSELECT s.* FROM @relation s LEFT JOIN {read} a USING (code)"


def _gate_model(sql: str, audits: str) -> str:
    return _model(sql, header=f'MODEL (description "Gated.", audits [{audits}]);')


# (description, files, project toml)
EXPLICIT_REFERENCE_PROJECTS: tuple[tuple[str, dict[str, str], str], ...] = (
    (
        "task literal SQL reading a managed source and a model",
        {
            **_MODELS,
            **_MANAGED,
            _HOOK_PATH: _hook(),
            _LOADER_PATH: _loaders(depends_on="", body=""),
            _TASK_PATH: _task(
                '    ctx.query("SELECT * FROM raw_customers JOIN customers ON true")\n'
                '    ctx.execute_sql(sql="DELETE FROM main.orders")\n'
            ),
        },
        _PROJECT,
    ),
    (
        "every owner kind, dedup on one line and line ordering across files",
        {
            **_MODELS,
            **_SEED,
            _HOOK_PATH: _hook(
                reads='model("customers")',
                body=(
                    '    ctx.query("SELECT * FROM orders JOIN customers ON true")\n'
                    '    ctx.query("SELECT * FROM country_codes, COUNTRY_CODES")\n'
                ),
            ),
            _TASK_PATH: _task(
                '    ctx.query("SELECT * FROM customers c1 JOIN customers c2 ON true")\n'
                "    ctx.query(**{'sql': 'SELECT * FROM \"Orders\"'})\n"
            ),
            _CHECK_PATH: _check('    ctx.query("SELECT count(*) FROM memory.main.customers")\n'),
            _ASSET_PATH: _asset('    context.query("SELECT 1 FROM orders")\n'),
        },
        _PROJECT,
    ),
    (
        "temporary tables, f-string placeholders, external and unparseable SQL",
        {
            **_MODELS,
            **_EXTERNAL,
            _HOOK_PATH: _hook(),
            _TASK_PATH: _task(
                '    ctx.query("CREATE TEMP TABLE customers AS SELECT 1")\n'
                '    ctx.query("SELECT * FROM customers JOIN main.customers ON true")\n'
                '    ctx.query(f"SELECT * FROM {ctx.schema}.orders '
                'JOIN warehouse.events ON true")\n'
                '    ctx.query(f"SELECT * FROM orders_{1} JOIN landing.orders_feed ON true")\n'
                '    ctx.query("SELECT * FROM computed JOIN external_lookup ON true")\n'
                '    ctx.query("SELEC broken FROM (")\n'
                "    ctx.query(sql_text)\n"
            ),
        },
        _PROJECT,
    ),
    (
        "loaders read their own, upstream and other sources and models",
        {
            **_MODELS,
            **_MANAGED,
            **_SEED,
            _HOOK_PATH: _hook(),
            _LOADER_PATH: _loaders(
                depends_on="raw_regions",
                body=(
                    '    ctx.query("SELECT * FROM raw_customers JOIN raw_regions ON true")\n'
                    '    ctx.query("SELECT * FROM customers JOIN country_codes ON true")\n'
                ),
            ),
        },
        _PROJECT,
    ),
    (
        "loader with no upstream loader reads another managed source",
        {
            **_MODELS,
            **_MANAGED,
            _HOOK_PATH: _hook(),
            _LOADER_PATH: _loaders(
                depends_on="",
                body='    ctx.query("SELECT * FROM raw_regions")\n',
            ),
        },
        _PROJECT,
    ),
    (
        "old names kept as compatibility views",
        {
            "models/marts/customers.sql": _model(
                "SELECT 1 AS customer_id",
                header=(
                    'MODEL (description "Customers.", materialized table, '
                    "migrate_from legacy_customers);"
                ),
            ),
            "models/marts/orders.sql": _model(
                "SELECT 1 AS order_id",
                header=(
                    'MODEL (description "Orders.", materialized table, '
                    'migrate_from "archive.old_orders");'
                ),
            ),
            "models/marts/regions.sql": _model(
                "SELECT 1 AS region_id",
                header=(
                    'MODEL (description "Regions.", materialized table, migrate_from customers);'
                ),
            ),
            _TASK_PATH: _task(
                '    ctx.query("SELECT * FROM legacy_customers JOIN archive.old_orders ON true")\n'
                '    ctx.query("SELECT * FROM main.LEGACY_CUSTOMERS")\n'
            ),
        },
        _PROJECT,
    ),
    (
        "old names off leave old relations unmatched",
        {
            "models/marts/customers.sql": _model(
                "SELECT 1 AS customer_id",
                header=(
                    'MODEL (description "Customers.", materialized table, '
                    "migrate_from legacy_customers);"
                ),
            ),
            "models/marts/orders.sql": _model("SELECT 1 AS order_id"),
            _TASK_PATH: _task('    ctx.query("SELECT * FROM legacy_customers")\n'),
        },
        _OLD_NAMES_OFF,
    ),
    (
        "enforcement disabled still checks hooks and audits",
        {
            **_MODELS,
            _HOOK_PATH: _hook(reads='model("customers")'),
            _TASK_PATH: _task('    ctx.query("SELECT * FROM customers")\n'),
        },
        _DISABLED,
    ),
    (
        "python hook declaring an unknown model read",
        {
            **_MODELS,
            _HOOK_PATH: _hook(reads='model("missing_customers")'),
            _TASK_PATH: _task('    ctx.query("SELECT * FROM customers")\n'),
        },
        _PROJECT,
    ),
    (
        "unknown hook reads raise for the first hook by name",
        {
            **_MODELS,
            _HOOK_PATH: (
                _hook(reads='(seed("nope"), model("customers"))', name="z_hook")
                + "\n\n"
                + _hook(reads='source("missing")', name="a_hook").split("\n\n\n", 1)[1]
                + "\n\n"
                + _hook(name="refresh_lookup").split("\n\n\n", 1)[1]
            ),
        },
        _PROJECT,
    ),
    (
        "python and sql hooks reading their own dependants",
        {
            _HOOK_PATH: _hook(reads='model("customers")'),
            _SQL_HOOK_PATH: (
                'HOOK (description "Records rows.");\n'
                'INSERT INTO audit_log SELECT count(*) FROM __ref("item_totals")\n'
            ),
            "models/marts/orders.sql": _model("SELECT 1 AS order_id", header=_HOOKED),
            "models/marts/customers.sql": _model('SELECT * FROM __ref("orders")'),
            "models/marts/items.sql": _model("SELECT 1 AS item_id", header=_NAMED_SQL_HOOK),
            "models/marts/item_totals.sql": _model(
                'SELECT * FROM __ref("items")',
                header=(
                    'MODEL (description "Totals.", pre_hooks '
                    "[inline_sql('SELECT * FROM __ref(\"item_rollup\")')]);"
                ),
            ),
            "models/marts/item_rollup.sql": _model('SELECT * FROM __ref("item_totals")'),
        },
        _PROJECT,
    ),
    (
        "attached audits reading their targets' dependants",
        {
            **_SEED,
            **_EXTERNAL,
            "audits/generic/gated_lookup.sql": _GATED_AUDIT.format(read='__ref("enriched")'),
            "audits/generic/source_lookup.sql": _GATED_AUDIT.format(read='__ref("landing_view")'),
            "models/marts/base.sql": _gate_model("SELECT 'GB' AS code", "gated_lookup"),
            "models/marts/enriched.sql": _model('SELECT * FROM __ref("base")'),
            "audits/generic/seed_lookup.sql": _GATED_AUDIT.format(read='__ref("coded")'),
            "models/marts/landing_view.sql": _model('SELECT * FROM __source("landing_audited")'),
            "models/marts/coded.sql": _model('SELECT * FROM __seed("audited_codes")'),
            "sources/landing_audits.yml": (
                "sources:\n  - name: landing_audited\n    description: Landing audited.\n"
                "    schema: landing\n    audits:\n      - source_lookup\n"
            ),
            "seeds/audited_codes.yml": (
                "seeds:\n  - name: audited_codes\n    description: Audited codes.\n"
                "    columns:\n      - name: code\n        type: VARCHAR\n"
                "    audits:\n      - seed_lookup\n"
            ),
            "seeds/audited_codes.csv": "code\nGB\n",
        },
        _PROJECT,
    ),
    (
        "audit gates, hook cycles and hard-coded names together",
        {
            "audits/generic/gated_lookup.sql": _GATED_AUDIT.format(read='__ref("enriched")'),
            "models/marts/base.sql": _gate_model("SELECT 'GB' AS code", "gated_lookup"),
            "models/marts/enriched.sql": _model('SELECT * FROM __ref("base")'),
            _HOOK_PATH: _hook(
                reads='model("customers")',
                body='    ctx.query("SELECT * FROM legacy_orders JOIN orders ON true")\n',
            ),
            "models/marts/orders.sql": _model(
                "SELECT 1 AS order_id",
                header=(
                    'MODEL (description "Orders.", materialized table, '
                    'migrate_from legacy_orders, post_hooks [python("refresh_lookup")]);'
                ),
            ),
            "models/marts/customers.sql": _model('SELECT * FROM __ref("orders")'),
            _TASK_PATH: _task('    ctx.query("SELECT * FROM enriched")\n'),
        },
        _PROJECT,
    ),
)


def write_project(*, project_dir: Path, files: dict[str, str], toml: str) -> None:
    """Write the project config and files below `project_dir`."""

    _ = [
        _write(path=project_dir / relative, contents=contents)
        for relative, contents in {"sqlbuild_project.toml": toml, **files}.items()
    ]


def compile_outcome(*, project_dir: Path) -> tuple[object, ...]:
    """Assemble the project: every diagnostic and unmatched relation, or the error raised."""

    try:
        project: CompiledProjectView = assemble_project(
            inputs=build_compile_inputs(
                discovered_inputs=discover_project_inputs(project_dir=project_dir),
                adapter_context=_ADAPTER_CONTEXT,
                run_id="explicit_references",
            ),
            skip_column_inference=True,
        )
    except Exception as error:
        return (
            "error",
            type(error).__name__,
            str(error),
            getattr(error, "code", None),
            getattr(error, "help", None),
        )
    return (
        "ok",
        tuple(_diagnostic_view(diagnostic) for diagnostic in project.diagnostics),
        tuple(_unmatched_view(relation) for relation in project.unmatched_literal_sql_relations),
    )


def _diagnostic_view(diagnostic: CompilerDiagnostic) -> tuple[object, ...]:
    return (
        str(diagnostic.phase),
        str(diagnostic.severity),
        diagnostic.code,
        diagnostic.message,
        _optional(diagnostic.path, Path.as_posix),
        diagnostic.line,
        _optional(diagnostic.location, attrgetter("column")),
        _optional(diagnostic.resource_type, str),
        diagnostic.resource_name,
        diagnostic.help,
    )


def _optional(value: Any, convert: Callable[[Any], object]) -> object:
    """`convert(value)`, or None where `value` is None."""

    return {True: lambda: None, False: lambda: convert(value)}[value is None]()


def outcome_codes(outcome: tuple[object, ...]) -> tuple[object, ...]:
    """The diagnostic codes of a successful outcome; none for a raised error."""

    return {
        "ok": lambda: tuple(diagnostic[2] for diagnostic in cast(Any, outcome[1])),
        "error": tuple,
    }[cast(str, outcome[0])]()


def _unmatched_view(relation: LiteralSqlRelation) -> tuple[object, ...]:
    return (
        relation.owner_label,
        str(relation.owner_kind),
        relation.relative_path.as_posix(),
        relation.line,
        relation.method,
        (relation.relation.database, relation.relation.schema, relation.relation.name),
    )


def _write(*, path: Path, contents: str) -> int:
    path.parent.mkdir(parents=True, exist_ok=True)
    return path.write_text(contents, encoding="utf-8")
