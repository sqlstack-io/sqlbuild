"""Minimal projects that fail inside project discovery, one per diagnostic family."""

import os

from scripts.compiler_differential._helpers.corpus.case_builder import (
    config_files,
    failure_case,
)
from scripts.compiler_differential.constants import (
    FAILURE_BASE_FILES,
    FAILURE_BASE_MART,
    FAILURE_BASE_STAGING,
    FAILURE_MART_PATH,
    FAILURE_SOURCES_PATH,
    FAILURE_STAGING_PATH,
    GENERATOR_BOM,
)
from scripts.compiler_differential.models import FailureCase

_TASK_MODULE: str = (
    "from sqlbuild.assets import asset\n\n\n"
    "@asset\ndef export_orders(ctx):\n"
    '    """Export orders."""\n'
    "    return None\n"
)
_HOOK_MODULE: str = (
    "from sqlbuild.hooks import hook\n\n\n"
    "@hook\ndef note_refresh(ctx):\n"
    '    """Log one refresh."""\n'
    "    return None\n"
)
_MACRO_MODULE: str = "def cents(value):\n    return f'{value} * 100'\n"
_DEEP_HEADER_DEPTH: int = 20_000
_DEEP_HEADER_VALUE: str = "[" * _DEEP_HEADER_DEPTH + '"placed"' + "]" * _DEEP_HEADER_DEPTH
_DEEP_HEADER_CALLS: str = "limit(level " * _DEEP_HEADER_DEPTH + "1" + ")" * _DEEP_HEADER_DEPTH
_DEEP_HEADER_MESSAGE: str = "contains invalid SQLBuild header syntax: values nest deeper than 256"
_SEED_DECLARATION: str = (
    "seeds:\n  - name: order_channels\n    description: Order channels.\n"
    "    columns:\n      - name: id\n        type: INTEGER\n"
    "      - name: label\n        type: VARCHAR\n"
)


def discovery_failure_cases() -> tuple[FailureCase, ...]:
    """Return the discovery-time failure cases in a stable order."""

    return (
        *_config_cases(),
        *_sql_file_cases(),
        *_layout_cases(),
        *_yaml_file_cases(),
        *_conflict_cases(),
        *_python_cases(),
    )


def _config_cases() -> tuple[FailureCase, ...]:
    return (
        failure_case(
            name="local-config-unknown-key",
            expected_code="D001",
            files={"sqlbuild_local.toml": "[settings]\nsql_analysiss = false\n"},
        ),
        failure_case(
            name="target-unknown-key",
            expected_code="D001",
            files=config_files('\n[targets.ci]\nschema = "ci"\nwarehouse_size = "large"\n'),
        ),
        failure_case(
            name="dbt-unknown-key",
            expected_code="D001",
            files=config_files('\n[dbt]\nproject_dir = "dbt"\nmanifest = "dbt/manifest.json"\n'),
        ),
        failure_case(
            name="path-defaults-unmatched",
            expected_code="D007",
            files=config_files('\n[path_defaults."reporting/finance"]\nmaterialized = "table"\n'),
        ),
    )


def _sql_file_cases() -> tuple[FailureCase, ...]:
    return (
        failure_case(
            name="model-bom",
            expected_code="D002",
            files={FAILURE_STAGING_PATH: GENERATOR_BOM + FAILURE_BASE_STAGING},
        ),
        failure_case(
            name="model-missing-header",
            expected_code="D002",
            files={FAILURE_STAGING_PATH: 'SELECT order_id\nFROM __source("raw_orders")\n'},
        ),
        failure_case(
            name="model-unknown-header-key",
            expected_code="D002",
            files={
                FAILURE_STAGING_PATH: FAILURE_BASE_STAGING.replace(
                    '"Staged orders",', '"Staged orders",\n  materialised table,'
                )
            },
        ),
        failure_case(
            name="model-header-nested-too-deep",
            expected_code="D002",
            files={
                FAILURE_STAGING_PATH: FAILURE_BASE_STAGING.replace(
                    '"Staged orders",', f'"Staged orders",\n  tags {_DEEP_HEADER_VALUE},'
                )
            },
            expected_message=_DEEP_HEADER_MESSAGE,
        ),
        failure_case(
            name="model-path-not-utf8",
            expected_code="D016",
            files={os.fsdecode(b"sql/models/staging/caf\xe9.sql"): FAILURE_BASE_STAGING},
            expected_message="is not valid UTF-8; rename it so SQLBuild can read it",
        ),
        failure_case(
            name="model-reserved-name",
            expected_code="D016",
            files={"sql/models/marts/_chain_.sql": FAILURE_BASE_MART},
        ),
        failure_case(
            name="test-case-missing-parameter",
            expected_code="D003",
            files={
                "tests/unit/test_order_cases.sql": (
                    'TEST (\n  name "keeps_status",\n'
                    "  parameters (\n    status_value string,\n    amount_value float,\n  ),\n"
                    '  cases (\n    placed (status_value "placed"),\n  ),\n);\n\n'
                    "WITH\n__source__raw_orders AS (\n"
                    '  SELECT 1 AS order_id, 10 AS customer_id, @param("amount_value") AS amount,'
                    ' @param("status_value") AS status\n),\n'
                    "__expected__stg_orders AS (\n  SELECT 1 AS order_id\n)\nSELECT 1\n"
                )
            },
        ),
        failure_case(
            name="generic-audit-header",
            expected_code="D004",
            files={
                "sql/models/marts/_sqlbuild/_audits/generic/at_least.sql": (
                    "AUDIT (unknown_option 1);\n\nSELECT * FROM @relation WHERE @column < 0\n"
                )
            },
        ),
        failure_case(
            name="list-constant-mixed-types",
            expected_code="D013",
            files={
                "sql/models/staging/_sqlbuild/_constants/statuses.sql": (
                    'CONSTANT (name kept_statuses, value ["placed", 3]);\n'
                )
            },
        ),
        failure_case(
            name="constant-header-nested-too-deep",
            expected_code="D013",
            files={
                "sql/models/staging/_sqlbuild/_constants/statuses.sql": (
                    f"CONSTANT (name kept_statuses, value {_DEEP_HEADER_CALLS});\n"
                )
            },
            expected_message=_DEEP_HEADER_MESSAGE,
        ),
        failure_case(
            name="enum-duplicate-member",
            expected_code="D013",
            files={
                "sql/models/staging/_sqlbuild/_enums/order_status.sql": (
                    "ENUM (\n  name order_status,\n  members [PLACED, PLACED],\n);\n"
                )
            },
        ),
        failure_case(
            name="sql-hook-unknown-key",
            expected_code="D014",
            files={
                "sql/hooks/sql/record_refresh.sql": (
                    'HOOK (\n  description "Record a refresh",\n  phase post,\n);\n\nSELECT 1\n'
                )
            },
        ),
        failure_case(
            name="sql-function-header",
            expected_code="D002",
            files={
                "sql/functions/sql/is_placed.sql": (
                    'FUNCTION (\n  description "Whether placed",\n  returns,\n);\n\n'
                    "order_status = 'placed'\n"
                )
            },
        ),
        failure_case(
            name="column-nullable-not-null",
            expected_code="P002",
            files={
                FAILURE_STAGING_PATH: FAILURE_BASE_STAGING.replace(
                    '"Staged orders",',
                    '"Staged orders",\n  columns (\n'
                    "    order_id (nullable true, audits [not_null]),\n  ),",
                )
            },
        ),
    )


def _layout_cases() -> tuple[FailureCase, ...]:
    return tuple(
        failure_case(name=name, expected_code="D013", files={path: contents})
        for name, path, contents in (
            ("scoped-root-at-project-root", "_enums/order_status.sql", ""),
            (
                "declaration-group-below-root",
                "sql/models/_sqlbuild/macros/python/money.py",
                _MACRO_MODULE,
            ),
            ("declaration-root-nested", "sql/models/marts/_enums/_constants/limits.sql", ""),
            ("declaration-group-unsupported-entry", "sql/models/marts/_sqlbuild/notes.txt", ""),
            ("named-role-nested-declaration", "sql/schemas/enums/order_status.sql", ""),
            ("audit-role-unsupported-entry", "sql/audits/at_least.sql", ""),
            ("local-singular-audit", "sql/models/marts/_sqlbuild/_audits/singular/at_least.sql", ""),
            ("hook-role-unsupported-entry", "sql/models/marts/_sqlbuild/hooks/notes.sql", ""),
        )
    )


def _yaml_file_cases() -> tuple[FailureCase, ...]:
    return (
        failure_case(
            name="schema-yml-model-metadata",
            expected_code="D005",
            files={
                "sql/models/marts/schema.yml": (
                    "models:\n  - name: customer_totals\n    description: Totals.\n"
                )
            },
        ),
        failure_case(
            name="seed-yaml-extension",
            expected_code="D005",
            files={
                "sql/seeds/order_channels.yaml": _SEED_DECLARATION,
                "sql/seeds/order_channels.csv": "id,label\n1,web\n",
            },
        ),
        failure_case(
            name="schema-yml-invalid-yaml",
            expected_code="D005",
            files={"sql/models/marts/schema.yml": "models: [\n"},
            expected_message="contains invalid YAML at line 2, column 1",
        ),
        failure_case(
            name="source-unsupported-yaml",
            expected_code="D006",
            files={
                FAILURE_SOURCES_PATH: FAILURE_BASE_FILES[FAILURE_SOURCES_PATH]
                + "    meta:\n      blob: !!binary aGk=\n"
            },
            expected_message="uses YAML that SQLBuild does not support at line",
        ),
        failure_case(
            name="source-unknown-key",
            expected_code="D006",
            files={
                FAILURE_SOURCES_PATH: FAILURE_BASE_FILES[FAILURE_SOURCES_PATH].replace(
                    "    description: Orders feed.\n",
                    "    description: Orders feed.\n    freshnes: 1d\n",
                )
            },
        ),
        failure_case(
            name="source-removed-ingestr-block",
            expected_code="D006",
            expected_message="declares an ingestr integration, which SQLBuild no longer supports",
            files={
                FAILURE_SOURCES_PATH: FAILURE_BASE_FILES[FAILURE_SOURCES_PATH].replace(
                    "    description: Orders feed.\n",
                    "    description: Orders feed.\n    ingestr:\n"
                    "      source_table: public.orders\n",
                )
            },
        ),
        failure_case(
            name="sources-removed-dlt-sources",
            expected_code="D006",
            expected_message="declares dlt_sources, a dlt integration",
            files={
                "sql/sources/loaded.yml": (
                    "dlt_sources:\n  - type: sql_database\n    resources:\n"
                    "      - name: loaded_orders\n        table: orders\n"
                )
            },
        ),
        failure_case(
            name="seed-declaration-without-csv",
            expected_code="D008",
            files={"sql/seeds/order_channels.yml": _SEED_DECLARATION},
        ),
        failure_case(
            name="seed-header-mismatch",
            expected_code="D008",
            files={
                "sql/seeds/order_channels.yml": _SEED_DECLARATION,
                "sql/seeds/order_channels.csv": "id,name\n1,web\n",
            },
        ),
    )


def _conflict_cases() -> tuple[FailureCase, ...]:
    return (
        failure_case(
            name="duplicate-source",
            expected_code="D007",
            files={"sql/sources/more.yml": FAILURE_BASE_FILES[FAILURE_SOURCES_PATH]},
        ),
        failure_case(
            name="duplicate-scenario",
            expected_code="D007",
            files={
                f"tests/scenarios/{folder}/orders.sql": (
                    'SCENARIO (description "Orders world");\n\n'
                    "WITH\n__source__raw_orders AS (\n"
                    "  SELECT 1 AS order_id, 10 AS customer_id, CAST(5 AS DOUBLE) AS amount,"
                    " 'placed' AS status\n),\n"
                    "__expected__stg_orders AS (\n  SELECT 1 AS order_id\n)\nSELECT 1\n"
                )
                for folder in ("north", "south")
            },
        ),
        failure_case(
            name="resource-name-collision",
            expected_code="D007",
            files={
                "sql/seeds/stg_orders.yml": _SEED_DECLARATION.replace("order_channels", "stg_orders"),
                "sql/seeds/stg_orders.csv": "id,label\n1,web\n",
            },
        ),
        failure_case(
            name="managed-source-without-loader",
            expected_code="D007",
            files={
                FAILURE_SOURCES_PATH: FAILURE_BASE_FILES[FAILURE_SOURCES_PATH]
                + "  - name: loaded_products\n    description: Loaded products.\n"
                "    managed: true\n    write_strategy: table\n"
                "    columns:\n      - name: id\n        type: INTEGER\n"
            },
        ),
        failure_case(
            name="duplicate-python-node",
            expected_code="D007",
            files={
                "python/nodes/tasks/first.py": _TASK_MODULE,
                "python/nodes/tasks/second.py": _TASK_MODULE,
            },
        ),
        failure_case(
            name="check-depends-on-check",
            expected_code="D007",
            files={
                "python/nodes/checks/orders.py": (
                    "from sqlbuild.checks import check\nfrom sqlbuild.refs import model\n\n\n"
                    '@check(depends_on=model("stg_orders"))\ndef orders_present(ctx):\n'
                    '    """Orders present."""\n    return ctx.pass_()\n\n\n'
                    "@check(depends_on=orders_present)\ndef orders_checked(ctx):\n"
                    '    """Orders checked."""\n    return ctx.pass_()\n'
                )
            },
        ),
    )


def _python_cases() -> tuple[FailureCase, ...]:
    return (
        failure_case(
            name="python-node-import-error",
            expected_code="D011",
            files={"python/nodes/tasks/exports.py": "import missing_orders_client\n" + _TASK_MODULE},
        ),
        failure_case(
            name="duplicate-hook",
            expected_code="D011",
            files={
                "sql/hooks/python/first.py": _HOOK_MODULE,
                "sql/hooks/python/second.py": _HOOK_MODULE,
            },
        ),
        failure_case(
            name="provider-invalid-settings",
            expected_code="D012",
            files={
                "python/providers/export_settings.py": (
                    "from sqlbuild.providers import Provider\n\n\n"
                    "class ExportSettings(Provider):\n"
                    '    """Export settings."""\n\n'
                    "    api_token: str\n"
                )
            },
        ),
        failure_case(
            name="duplicate-provider",
            expected_code="D012",
            files={
                f"python/providers/{module}.py": (
                    "from sqlbuild.providers import Provider\n\n\n"
                    "class ExportSettings(Provider):\n"
                    '    """Export settings."""\n\n'
                    '    region: str = "north"\n'
                )
                for module in ("first", "second")
            },
        ),
        failure_case(
            name="orchestration-python-job-module",
            expected_code="D017",
            files={
                "orchestration/jobs.py": (
                    "from sqlbuild.orchestration import job\n"
                    "nightly = job('nightly', description='Orders.', select='stg_orders')\n"
                )
            },
            expected_message="files cannot sit directly in orchestration/",
        ),
        failure_case(
            name="legacy-event-exporters-directory",
            expected_code="D015",
            files={"event_exporters/publish.py": "VALUE = 1\n"},
        ),
        failure_case(
            name="lifecycle-sink-default-parameter",
            expected_code="D015",
            files={
                "python/event_sinks/lifecycle.py": (
                    "from sqlbuild.event_sinks import LifecycleEvent, lifecycle_event_sink\n\n\n"
                    "@lifecycle_event_sink()\n"
                    "def ignore_events(event: LifecycleEvent, limit=3) -> None:\n"
                    "    return None\n"
                )
            },
        ),
        failure_case(
            name="lifecycle-sink-unknown-provider",
            expected_code="D015",
            files={
                "python/event_sinks/lifecycle.py": (
                    "from sqlbuild.event_sinks import LifecycleEvent, lifecycle_event_sink\n\n\n"
                    "@lifecycle_event_sink()\n"
                    "def publish_events(event: LifecycleEvent, destination_client) -> None:\n"
                    "    return None\n"
                )
            },
        ),
        failure_case(
            name="command-output-sink-unknown-provider",
            expected_code="D015",
            files={
                "python/event_sinks/output.py": (
                    "from sqlbuild.event_sinks import CommandOutputRecord, command_output_event_sink\n\n\n"
                    '@command_output_event_sink(streams={"stdout"})\n'
                    "def publish_output(record: CommandOutputRecord, destination_client) -> None:\n"
                    "    return None\n"
                )
            },
        ),
        failure_case(
            name="materialization-import-error",
            expected_code="D011",
            files={
                "python/materializations/copy_table.py": "import missing_warehouse_client\n",
                FAILURE_MART_PATH: FAILURE_BASE_MART.replace(
                    '"Order totals per customer",',
                    '"Order totals per customer",\n  materialized copy_table,',
                ),
            },
        ),
        failure_case(
            name="python-outside-extension-root",
            expected_code="D016",
            files={"helpers/formatting.py": "VALUE = 1\n"},
        ),
    )
