"""Minimal failing projects, each derived from the shared base project."""

from scripts.compiler_differential._helpers.analysis_corpus.failure_cases import (
    analysis_failure_cases,
)
from scripts.compiler_differential._helpers.corpus.attachment_failure_cases import (
    attachment_failure_cases,
)
from scripts.compiler_differential._helpers.corpus.case_builder import (
    failure_case,
    mart_body_files,
    staging_files,
)
from scripts.compiler_differential._helpers.corpus.config_failure_cases import (
    config_failure_cases,
)
from scripts.compiler_differential._helpers.corpus.discovery_failure_cases import (
    discovery_failure_cases,
)
from scripts.compiler_differential._helpers.corpus.reference_failure_cases import (
    reference_failure_cases,
)
from scripts.compiler_differential._helpers.corpus.render_failure_cases import (
    render_failure_cases,
)
from scripts.compiler_differential._helpers.corpus.scope_failure_cases import (
    scope_failure_cases,
)
from scripts.compiler_differential.constants import (
    FAILURE_BASE_CONFIG,
    FAILURE_BASE_MART,
    FAILURE_BASE_STAGING,
    FAILURE_CONFIG_PATH,
    FAILURE_MART_PATH,
    FAILURE_SOURCES_PATH,
    FAILURE_UNDECODABLE_ENV_VAR,
)
from scripts.compiler_differential.models import FailureCase

_MART_HEADER_START: str = 'MODEL (\n  description "Order totals per customer",\n'

_DESCRIPTION_QUOTE_HELP: str = (
    'write the text between triple quotes, where quotes need no escaping: description """Matches '
    '"Miss"."""; sqb format --fix rewrites a description that escapes its quotes as \\" '
    'or doubles them as ""'
)

_RULES_CONFIG: str = '\n[rules]\nselect = ["{codes}"]\n'

_ENGINE_ERROR_MACROS: str = "macros/python/cents.py"
_ENGINE_ERROR_CENTS: str = (
    'def cents(expression: str) -> str:\n    """Convert to cents."""\n'
    '    return f"({expression} * 100)"\n'
)
_MACRO_LITERALS_HELP: str = (
    "Macro arguments are Python literals (strings, numbers, True, False, None, lists, "
    "tuples and dicts), nested macro calls, and __ref(), __source() or __seed() "
    "references; compute anything else inside the macro"
)


def _rules(*codes: str) -> dict[str, str]:
    return {
        FAILURE_CONFIG_PATH: FAILURE_BASE_CONFIG + _RULES_CONFIG.format(codes='", "'.join(codes))
    }


def all_failure_cases() -> tuple[FailureCase, ...]:
    """Return every failure case in a stable order."""

    return (
        *_compile_failure_cases(),
        *discovery_failure_cases(),
        *render_failure_cases(),
        *config_failure_cases(),
        *scope_failure_cases(),
        *reference_failure_cases(),
        *attachment_failure_cases(),
        *analysis_failure_cases(),
        *engine_error_cases(),
    )


def _compile_failure_cases() -> tuple[FailureCase, ...]:
    return (
        failure_case(
            name="config-toml-syntax",
            expected_code="D001",
            files={
                FAILURE_CONFIG_PATH: FAILURE_BASE_CONFIG + "[defaults\nmaterialized = 'table'\n"
            },
        ),
        failure_case(
            name="config-unknown-default-key",
            expected_code="D001",
            files={
                FAILURE_CONFIG_PATH: FAILURE_BASE_CONFIG + '\n[defaults]\ndescription = "Shared"\n'
            },
        ),
        failure_case(
            name="config-unknown-adapter",
            expected_code="C601",
            files={FAILURE_CONFIG_PATH: FAILURE_BASE_CONFIG.replace('"duckdb"', '"warehouse9000"')},
        ),
        failure_case(
            name="model-header-syntax",
            expected_code="D002",
            files=staging_files(
                FAILURE_BASE_STAGING.replace(
                    '"Staged orders",', '"Staged orders",\n  materialized (,'
                )
            ),
        ),
        failure_case(
            name="model-sql-syntax",
            expected_code="P001",
            files={
                FAILURE_CONFIG_PATH: FAILURE_BASE_CONFIG
                + "\n[settings]\nrequire_sql_analysis = true\n",
                **staging_files(
                    FAILURE_BASE_STAGING.replace("SELECT order_id,", "SELECT order_id,, ")
                ),
            },
        ),
        failure_case(
            name="test-parse",
            expected_code="D003",
            files={"tests/unit/test_stg_orders.sql": "TEST(\n\nSELECT 1\n"},
        ),
        failure_case(
            name="audit-parse",
            expected_code="D004",
            files={
                "models/marts/_sqlbuild/audits/singular/broken.sql": "AUDIT (name);\n\nSELECT 1\n"
            },
        ),
        failure_case(
            name="schema-unterminated-declaration",
            expected_code="D013",
            files={"models/staging/_sqlbuild/_schemas/orders.sql": "SCHEMA (\n  name orders,\n"},
        ),
        failure_case(
            name="source-parse",
            expected_code="D006",
            files={FAILURE_SOURCES_PATH: "sources:\n  - name: raw_orders\n    columns: [\n"},
        ),
        failure_case(
            name="duplicate-model",
            expected_code="D007",
            files={"models/marts/stg_orders.sql": FAILURE_BASE_STAGING},
        ),
        failure_case(
            name="seed-without-header",
            expected_code="D008",
            files={"seeds/lookups.csv": ""},
        ),
        failure_case(
            name="scenario-parse",
            expected_code="D009",
            files={"tests/scenarios/orders.sql": "SCENARIO (\n\nSELECT 1\n"},
        ),
        failure_case(
            name="python-node-missing-description",
            expected_code="P010",
            files={
                "python/tasks/exports.py": (
                    "from sqlbuild.tasks import TaskContext, task\n\n\n"
                    "@task\ndef export_orders(ctx: TaskContext):\n    return None\n"
                )
            },
        ),
        failure_case(
            name="python-node-description-too-long",
            expected_code="P027",
            files={
                "python/tasks/exports.py": (
                    "from sqlbuild.tasks import TaskContext, task\n\n\n"
                    "@task\ndef export_orders(ctx: TaskContext):\n"
                    '    """'
                    + "\n\n    ".join(f"Orders paragraph {index}." for index in range(6))
                    + '"""\n    return None\n'
                )
            },
        ),
        failure_case(
            name="enum-parse",
            expected_code="D013",
            files={"enums/order_status.sql": "ENUM (\n  name order_status,\n  members [,\n);\n"},
        ),
        failure_case(
            name="hook-without-header",
            expected_code="D014",
            files={"hooks/sql/record_refresh.sql": "SELECT 1\n"},
        ),
        failure_case(
            name="invalid-resource-name",
            expected_code="D016",
            files={"models/marts/order-totals.sql": FAILURE_BASE_MART},
        ),
        failure_case(
            name="built-in-audit-shadow",
            expected_code="S010",
            expected_warning_code="P003",
            files={
                "models/staging/_sqlbuild/_audits/generic/not_null.sql": (
                    'AUDIT (name "not_null");\n\nSELECT 1 WHERE FALSE\n'
                )
            },
        ),
        failure_case(
            name="singular-audit-one-resource",
            expected_code="P004",
            files={
                "models/marts/_sqlbuild/audits/singular/positive_totals.sql": (
                    'AUDIT (name "positive_totals");\n\n'
                    'SELECT customer_id FROM __ref("customer_totals") WHERE total_amount < 0\n'
                )
            },
        ),
        failure_case(
            name="macro-generated-reference",
            expected_code="P006",
            files={
                "models/marts/_sqlbuild/_macros/python/orders.py": (
                    "def staged_orders() -> str:\n"
                    '    """Return the staged orders relation."""\n'
                    "    return '__ref(\"stg_orders\")'\n"
                ),
                **mart_body_files(
                    "SELECT customer_id, SUM(amount) AS total_amount\n"
                    "FROM @py.staged_orders()\nGROUP BY customer_id\n"
                ),
            },
        ),
        failure_case(
            name="missing-description",
            expected_code="P010",
            files=staging_files(
                FAILURE_BASE_STAGING.replace('  description "Staged orders",\n', "")
            ),
        ),
        failure_case(
            name="unknown-ref",
            expected_code="P001",
            files=mart_body_files(
                'SELECT customer_id, SUM(amount) AS total_amount\nFROM __ref("stg_payments")\n'
                "GROUP BY customer_id\n"
            ),
        ),
        failure_case(
            name="unknown-source",
            expected_code="P001",
            files=staging_files(
                FAILURE_BASE_STAGING.replace('__source("raw_orders")', '__source("raw_payments")')
            ),
        ),
        failure_case(
            name="unknown-macro",
            expected_code="P001",
            files=staging_files(
                FAILURE_BASE_STAGING.replace(
                    "amount, status", '@py.cents("amount") AS amount, status'
                )
            ),
        ),
        failure_case(
            name="failing-macro",
            expected_code="P001",
            files={
                "models/staging/_sqlbuild/_macros/python/cents.py": (
                    "def cents(expression: str) -> str:\n"
                    '    """Refuse every input."""\n'
                    '    raise ValueError(f"cannot convert {expression}")\n'
                ),
                **staging_files(
                    FAILURE_BASE_STAGING.replace(
                        "amount, status", '@py.cents("amount") AS amount, status'
                    )
                ),
            },
        ),
        failure_case(
            name="unknown-enum-member",
            expected_code="P001",
            files={
                "models/staging/_sqlbuild/_enums/order_status.sql": (
                    "ENUM (\n  name order_status,\n  members [PLACED, SHIPPED],\n);\n"
                ),
                **staging_files(
                    FAILURE_BASE_STAGING + 'WHERE status = @enum("order_status").RETURNED\n'
                ),
            },
        ),
        failure_case(
            name="unknown-constant",
            expected_code="P001",
            files=staging_files(FAILURE_BASE_STAGING + 'WHERE amount > @const("minimum_amount")\n'),
        ),
        failure_case(
            name="invalid-decimal-constant",
            expected_code="D013",
            files={
                "models/staging/_sqlbuild/_constants/limits.sql": (
                    'CONSTANT (name minimum_amount, type decimal, value "ten");\n'
                ),
                **staging_files(FAILURE_BASE_STAGING + 'WHERE amount > @const("minimum_amount")\n'),
            },
        ),
        failure_case(
            name="duplicate-declaration",
            expected_code="P001",
            files={
                "models/staging/_sqlbuild/_constants/limits.sql": (
                    "CONSTANT (name minimum_amount, value 1);\n"
                    "CONSTANT (name minimum_amount, value 2);\n"
                ),
                **staging_files(FAILURE_BASE_STAGING + 'WHERE amount > @const("minimum_amount")\n'),
            },
        ),
        failure_case(
            name="inaccessible-declaration",
            expected_code="P001",
            files={
                "models/marts/_sqlbuild/_constants/limits.sql": (
                    "CONSTANT (name minimum_amount, value 1);\n"
                ),
                **staging_files(FAILURE_BASE_STAGING + 'WHERE amount > @const("minimum_amount")\n'),
            },
        ),
        failure_case(
            name="over-broad-global",
            expected_code="S024",
            files={
                "constants/limits.sql": "CONSTANT (name minimum_amount, value 1);\n",
                **staging_files(FAILURE_BASE_STAGING + 'WHERE amount > @const("minimum_amount")\n'),
            },
        ),
        failure_case(
            name="unused-declaration",
            expected_code="S010",
            files={
                "models/staging/_sqlbuild/_constants/limits.sql": (
                    "CONSTANT (name spare, value 1);\n"
                )
            },
        ),
        failure_case(
            name="macro-import-cycle",
            expected_code="P001",
            files={
                "macros/python/first.py": (
                    "from macros.python.second import second_value\n\n\n"
                    "def first_value() -> str:\n"
                    '    """First value."""\n'
                    "    return second_value()\n"
                ),
                "macros/python/second.py": (
                    "from macros.python.first import first_value\n\n\n"
                    "def second_value() -> str:\n"
                    '    """Second value."""\n'
                    "    return first_value()\n"
                ),
            },
        ),
        failure_case(
            name="contract-missing-column",
            expected_code="K001",
            files=staging_files(
                'MODEL (\n  description "Staged orders",\n  contract enforced,\n'
                "  columns (\n    order_id (type INTEGER),\n    customer_id (type INTEGER),\n"
                "    amount (type DOUBLE),\n    status (type VARCHAR),\n"
                "    discount (type DOUBLE),\n"
                "  ),\n);\n\n"
                'SELECT order_id, customer_id, amount, status\nFROM __source("raw_orders")\n'
            ),
        ),
        failure_case(
            name="contract-type-mismatch",
            expected_code="K002",
            files=staging_files(
                'MODEL (\n  description "Staged orders",\n  contract enforced,\n'
                "  columns (\n    order_id (type VARCHAR),\n    customer_id (type INTEGER),\n"
                "    amount (type DOUBLE),\n    status (type VARCHAR),\n  ),\n);\n\n"
                'SELECT order_id, customer_id, amount, status\nFROM __source("raw_orders")\n'
            ),
        ),
        failure_case(
            name="set-operation-width",
            expected_code="B216",
            files=mart_body_files(
                'SELECT customer_id, amount FROM __ref("stg_orders")\n'
                'UNION ALL\nSELECT customer_id FROM __ref("stg_orders")\n'
            ),
        ),
        failure_case(
            name="comparison-type",
            expected_code="B217",
            files=mart_body_files(
                "SELECT customer_id, SUM(amount) AS total_amount\n"
                "FROM __ref(\"stg_orders\")\nWHERE amount = DATE '2026-01-01'\n"
                "GROUP BY customer_id\n"
            ),
        ),
        failure_case(
            name="unknown-column",
            expected_code="B002",
            files=mart_body_files(
                'SELECT customer_id, SUM(discount) AS total_amount\nFROM __ref("stg_orders")\n'
                "GROUP BY customer_id\n"
            ),
        ),
        failure_case(
            name="ungrouped-column",
            expected_code="B230",
            files=mart_body_files(
                "SELECT customer_id, status, SUM(amount) AS total_amount\n"
                'FROM __ref("stg_orders")\n'
                "GROUP BY customer_id\n"
            ),
        ),
        failure_case(
            name="test-expected-unknown-column",
            expected_code="B302",
            files={
                "tests/unit/test_stg_orders.sql": (
                    "TEST();\n\nWITH\n__source__raw_orders AS (\n"
                    "  SELECT 1 AS order_id, 10 AS customer_id, CAST(5 AS DOUBLE) AS amount,"
                    " 'placed' AS status\n),\n"
                    "__expected__stg_orders AS (\n  SELECT 1 AS order_id, 2 AS discount\n)\n"
                    "SELECT 1\n"
                )
            },
        ),
        failure_case(
            name="rule-unused-cte-after-variable-and-macro",
            expected_code="SQLCLEAN005",
            files={
                FAILURE_CONFIG_PATH: FAILURE_BASE_CONFIG
                + '\n[vars]\nfloor = "2"\n'
                + _RULES_CONFIG.format(codes="SQLCLEAN005"),
                "models/marts/_macros/python/cents.py": _ENGINE_ERROR_CENTS,
                **mart_body_files(
                    'WITH totals AS (SELECT customer_id, @py.cents("amount") AS cents, '
                    '@@floor AS floor_cents FROM __ref("stg_orders")), spare AS (SELECT 1 AS one)\n'
                    "SELECT customer_id, SUM(cents) AS total_amount\n"
                    "FROM totals WHERE cents > floor_cents\nGROUP BY customer_id\n"
                ),
            },
        ),
        failure_case(
            name="rule-unused-cte",
            expected_code="SQLCLEAN005",
            files={
                **_rules("SQLCLEAN005"),
                **mart_body_files(
                    "WITH spare AS (SELECT 1 AS one)\n"
                    "SELECT customer_id, SUM(amount) AS total_amount\n"
                    'FROM __ref("stg_orders")\nGROUP BY customer_id\n'
                ),
            },
        ),
        failure_case(
            name="rule-custom-finding",
            expected_code="XSQBRDIFF001",
            files={
                FAILURE_CONFIG_PATH: FAILURE_BASE_CONFIG
                + '\n[rules]\nselect = ["XSQBR"]\n\n'
                + "[rules.thresholds]\nmin_custom_rule_test_cases = 0\n",
                "rules/no_staging.py": (
                    "from sqlbuild.rules import Finding, Model, RuleContext, rule\n\n\n"
                    '@rule(code="XSQBRDIFF001", message="Models avoid the stg_ prefix", '
                    'remediation="Rename the model.")\n'
                    "def no_staging_prefix(*, model: Model, ctx: RuleContext) -> list[Finding]:\n"
                    '    if not model.name.startswith("stg_"):\n'
                    "        return []\n"
                    "    return [ctx.finding(subject=model)]\n"
                ),
            },
        ),
        failure_case(
            name="multiple-errors-order",
            expected_code="P010",
            files={
                **staging_files(
                    FAILURE_BASE_STAGING.replace('  description "Staged orders",\n', "")
                ),
                FAILURE_MART_PATH: FAILURE_BASE_MART.replace(
                    '  description "Order totals per customer",\n', ""
                ),
                "models/marts/order_counts.sql": (
                    "MODEL ();\n\nSELECT customer_id, COUNT(*) AS order_count\n"
                    'FROM __ref("stg_orders")\nGROUP BY customer_id\n'
                ),
            },
        ),
        failure_case(
            name="contract-extra-column",
            expected_code="K005",
            files=staging_files(
                'MODEL (\n  description "Staged orders",\n  contract enforced,\n'
                "  columns (\n    order_id (type INTEGER),\n    customer_id (type INTEGER),\n"
                "    amount (type DOUBLE),\n  ),\n);\n\n"
                'SELECT order_id, customer_id, amount, status\nFROM __source("raw_orders")\n'
            ),
        ),
        failure_case(
            name="contract-unknown-type",
            expected_code="K002",
            files=staging_files(
                'MODEL (\n  description "Staged orders",\n'
                "  columns (\n    order_id (type MYSTERY_TYPE),\n  ),\n);\n\n"
                'SELECT order_id, customer_id, amount, status\nFROM __source("raw_orders")\n'
            ),
        ),
        failure_case(
            name="cursor-unknown-column",
            expected_code="B300",
            files=mart_body_files(
                "SELECT customer_id, SUM(amount) AS total_amount\n"
                'FROM __ref("stg_orders")\nGROUP BY customer_id\n'
            )
            | {
                FAILURE_MART_PATH: (
                    'MODEL (\n  description "Order totals per customer",\n'
                    "  materialized incremental,\n  incremental_strategy delete_insert,\n"
                    "  cursor placed_at,\n  cursor_type timestamp,\n  cursor_grain day,\n"
                    "  cursor_inputs (\n    stg_orders order_id,\n  ),\n);\n\n"
                    "SELECT customer_id, SUM(amount) AS total_amount\n"
                    'FROM __ref("stg_orders")\nGROUP BY customer_id\n'
                )
            },
        ),
        failure_case(
            name="cursor-model-without-inputs",
            expected_code="P011",
            files={
                FAILURE_MART_PATH: (
                    'MODEL (\n  description "Order totals per customer",\n'
                    "  materialized incremental,\n  incremental_strategy delete_insert,\n"
                    "  cursor placed_at,\n  cursor_type timestamp,\n  cursor_grain day,\n);\n\n"
                    "SELECT TIMESTAMP '2026-01-01 00:00:00' AS placed_at, 1 AS customer_id\n"
                )
            },
        ),
        failure_case(
            name="unquoted-ref-name",
            expected_code="P012",
            expected_message="__ref(stg_orders) is not a valid __ref() call",
            files={
                FAILURE_MART_PATH: FAILURE_BASE_MART.replace(
                    '__ref("stg_orders")', "__ref(stg_orders)"
                )
            },
        ),
        failure_case(
            name="commented-ref-argument",
            expected_code="P012",
            expected_message="is not a valid __ref() call",
            files={
                FAILURE_MART_PATH: FAILURE_BASE_MART.replace(
                    '__ref("stg_orders")', "__ref( /* upstream */ 'stg_orders')"
                )
            },
        ),
        failure_case(
            name="single-quoted-source-name",
            expected_code="P012",
            expected_message="__source('raw_orders') is not a valid __source() call",
            files=staging_files(
                FAILURE_BASE_STAGING.replace('__source("raw_orders")', "__source('raw_orders')")
            ),
        ),
        failure_case(
            name="invalid-reference-calls-in-two-files",
            expected_code="P012",
            expected_message="__ref(stg_orders) is not a valid __ref() call",
            files={
                **staging_files(
                    FAILURE_BASE_STAGING.replace('__source("raw_orders")', "__source('raw_orders')")
                ),
                FAILURE_MART_PATH: FAILURE_BASE_MART.replace(
                    '__ref("stg_orders")', "__ref(stg_orders)"
                ),
            },
        ),
        failure_case(
            name="macro-returned-invalid-reference-call",
            expected_code="P012",
            expected_message=(
                "__ref(stg_orders) is not a valid __ref() call, returned by macro staged_orders()"
            ),
            files={
                "models/marts/_sqlbuild/_macros/python/orders.py": (
                    "def staged_orders() -> str:\n"
                    '    """Return the staged orders relation."""\n'
                    "    return '__ref(stg_orders)'\n"
                ),
                **mart_body_files(
                    "SELECT customer_id, SUM(amount) AS total_amount\n"
                    "FROM @py.staged_orders()\nGROUP BY customer_id\n"
                ),
            },
        ),
        failure_case(
            name="invalid-declaration-name",
            expected_code="D016",
            files={
                "models/staging/_sqlbuild/_constants/limits.sql": (
                    "CONSTANT (name MinimumAmount, value 1);\n"
                ),
                **staging_files(FAILURE_BASE_STAGING + 'WHERE amount > @const("MinimumAmount")\n'),
            },
        ),
        failure_case(
            name="python-node-syntax",
            expected_code="D011",
            files={"python/tasks/exports.py": "def broken(:\n    pass\n"},
        ),
        failure_case(
            name="hook-unknown-argument",
            expected_code="P001",
            files={
                "hooks/sql/record_refresh.sql": (
                    "HOOK (\n  description \"Record a refresh\"\n);\n\nSELECT @'label' AS label\n"
                ),
                FAILURE_MART_PATH: FAILURE_BASE_MART.replace(
                    '"Order totals per customer",',
                    '"Order totals per customer",\n  post_hooks [sql("record_refresh", tag: "x")],',
                ),
            },
        ),
        failure_case(
            name="model-private-enum-misuse",
            expected_code="P001",
            files=staging_files(
                'MODEL (\n  description "Staged orders",\n'
                "  enums (\n    _state [OPEN, CLOSED],\n  ),\n);\n\n"
                "SELECT order_id, customer_id, amount, status\n"
                'FROM __source("raw_orders")\nWHERE status = @enum("_state").PENDING\n'
            ),
        ),
        failure_case(
            name="variable-not-defined",
            expected_code="P001",
            files=staging_files(
                FAILURE_BASE_STAGING.replace("status\n", "status, '@@region' AS region\n", 1)
            ),
        ),
    )


def engine_error_cases() -> tuple[FailureCase, ...]:
    """User-facing errors that must be reported with the exact text; see the README."""

    return (
        failure_case(
            name="engine-error-unset-environment-variable",
            expected_code="P001",
            files=mart_body_files(
                "SELECT customer_id, '@@ENV:SQB_ENGINE_ERROR_UNSET_REGION' AS region\n"
                'FROM __ref("stg_orders")\n'
            ),
            expected_message=(
                "unknown environment variable '@@ENV:SQB_ENGINE_ERROR_UNSET_REGION' in "
                f"'<project>/{FAILURE_MART_PATH}'"
            ),
        ),
        failure_case(
            name="engine-error-case-folded-duplicate-column",
            expected_code="P001",
            files={
                FAILURE_MART_PATH: _MART_HEADER_START
                + '  columns (\n    Größe (description "Size"),\n'
                + '    größe (description "Size again"),\n  ),\n);\n\n'
                + 'SELECT customer_id AS größe\nFROM __ref("stg_orders")\n'
            },
            expected_message=(
                f"{FAILURE_MART_PATH} model has duplicate column 'größe' "
                "(column names are case-insensitive)"
            ),
        ),
        failure_case(
            name="engine-error-non-ascii-digit-constant-value",
            expected_code="D013",
            files={"constants/limits.sql": "CONSTANT (name minimum_amount, value \u0663);\n"},
            expected_message=(
                "<project>/constants/limits.sql has the bare number '\u0663', written with "
                "non-ASCII digits"
            ),
            expected_help=(
                'Quote it to keep it as text ("\u0663"), or write the number with ASCII digits 0-9'
            ),
        ),
        failure_case(
            name="engine-error-description-escaped-quote",
            expected_code="D002",
            files={
                FAILURE_MART_PATH: FAILURE_BASE_MART.replace(
                    'description "Order totals per customer"',
                    'description "Order totals per \\"customer\\""',
                    1,
                )
            },
            expected_message=(
                f"MODEL(...) in '<project>/{FAILURE_MART_PATH}:2:34' contains invalid SQLBuild "
                'header syntax: a double quote ends this "..." string early; use """…""" for '
                "text containing double quotes"
            ),
            expected_help=_DESCRIPTION_QUOTE_HELP,
        ),
        failure_case(
            name="engine-error-macro-description-escaped-quote",
            expected_code="P015",
            files={
                "macros/sql/customer_label.sql": (
                    "MACRO (\n"
                    "  description \"A customer's label,\n"
                    'quoted (\\"VIP\\").",\n'
                    "  arguments (customer_id BIGINT),\n"
                    "  returns VARCHAR,\n"
                    ");\n\n"
                    "CAST(@customer_id AS VARCHAR)\n"
                ),
                **mart_body_files(
                    "SELECT customer_id, @sql.customer_label(customer_id) AS label\n"
                    'FROM __ref("stg_orders")\n'
                ),
            },
            expected_message=(
                'a double quote ends this "..." string early; use """…""" for text '
                "containing double quotes\n  --> macros/sql/customer_label.sql:3:10"
            ),
            expected_help=_DESCRIPTION_QUOTE_HELP,
        ),
        failure_case(
            name="engine-error-non-ascii-digit-bare-number",
            expected_code="D013",
            files={
                "enums/order_priority.sql": (
                    "ENUM (\n  name order_priority,\n  members (LOW 1, HIGH \u0663),\n);\n"
                )
            },
            expected_message=(
                "<project>/enums/order_priority.sql has the bare number '\u0663', written with "
                "non-ASCII digits"
            ),
            expected_help=(
                'Quote it to keep it as text ("\u0663"), or write the number with ASCII digits 0-9'
            ),
        ),
        failure_case(
            name="engine-error-non-ascii-digit-model-constant",
            expected_code="D002",
            files={
                FAILURE_MART_PATH: FAILURE_BASE_MART.replace(
                    "MODEL (\n", "MODEL (\n  constants (_bonus \u0663),\n", 1
                )
            },
            expected_message=(
                f"<project>/{FAILURE_MART_PATH} has the bare number '\u0663', written with "
                "non-ASCII digits"
            ),
            expected_help=(
                'Quote it to keep it as text ("\u0663"), or write the number with ASCII digits 0-9'
            ),
        ),
        failure_case(
            name="engine-error-macro-argument-syntax",
            expected_code="P001",
            files={
                _ENGINE_ERROR_MACROS: _ENGINE_ERROR_CENTS,
                **mart_body_files(
                    "SELECT customer_id, @py.cents('amount',, 2) AS cents\n"
                    'FROM __ref("stg_orders")\n'
                ),
            },
            expected_message=(
                f"Macro arguments of '@py.cents' in '<project>/{FAILURE_MART_PATH}' "
                "could not be parsed: a value is missing here at line 5, column 40"
            ),
            expected_location=(5, 40),
            expected_help=_MACRO_LITERALS_HELP,
        ),
        failure_case(
            name="engine-error-macro-argument-second-call",
            expected_code="P001",
            files={
                _ENGINE_ERROR_MACROS: _ENGINE_ERROR_CENTS,
                **mart_body_files(
                    "SELECT customer_id, @py.cents('amount') AS cents,\n"
                    "  @py.cents('amount' 2) AS more_cents\n"
                    'FROM __ref("stg_orders")\n'
                ),
            },
            expected_message=(
                f"Macro arguments of '@py.cents' in '<project>/{FAILURE_MART_PATH}' "
                "could not be parsed: a comma is missing between arguments at line 6, column 22"
            ),
            expected_location=(6, 22),
            expected_help=(
                "Separate arguments, and the items of lists, tuples and dicts, with commas, for "
                "example @py.cents('amount', 2)"
            ),
        ),
        failure_case(
            name="engine-error-macro-argument-nested-call-sign",
            expected_code="P001",
            files={
                _ENGINE_ERROR_MACROS: _ENGINE_ERROR_CENTS,
                **mart_body_files(
                    "SELECT customer_id, @py.cents(-@py.cents('amount')) AS cents\n"
                    'FROM __ref("stg_orders")\n'
                ),
            },
            expected_message=(
                f"Macro arguments of '@py.cents' in '<project>/{FAILURE_MART_PATH}' apply unary - "
                "to the value of a nested macro call, which is a str and not a number at line 5, "
                "column 32"
            ),
            expected_location=(5, 32),
            expected_help=(
                "Return a number from the nested macro, or apply the sign inside the macro"
            ),
        ),
        failure_case(
            name="engine-error-macro-argument-named-sequence",
            expected_code="P001",
            files={
                _ENGINE_ERROR_MACROS: _ENGINE_ERROR_CENTS,
                **mart_body_files(
                    "SELECT customer_id, "
                    "@py.cents('\\N{LATIN CAPITAL LETTER A WITH MACRON AND GRAVE}') AS cents\n"
                    'FROM __ref("stg_orders")\n'
                ),
            },
            expected_message=(
                f"Macro arguments of '@py.cents' in '<project>/{FAILURE_MART_PATH}' could not be "
                "parsed: \\N{LATIN CAPITAL LETTER A WITH MACRON AND GRAVE} names a sequence of 2 "
                "characters, and \\N escapes name one character at line 5, column 32"
            ),
            expected_location=(5, 32),
            expected_help=(
                "Write each character of the sequence with its own \\N{...} escape, or write the "
                "characters themselves"
            ),
        ),
        failure_case(
            name="engine-error-macro-argument-surrogate-pair",
            expected_code="P001",
            files={
                _ENGINE_ERROR_MACROS: _ENGINE_ERROR_CENTS,
                **mart_body_files(
                    "SELECT customer_id, @py.cents('\\ud83d\\ude00') AS cents\n"
                    'FROM __ref("stg_orders")\n'
                ),
            },
            expected_message=(
                f"Macro arguments of '@py.cents' in '<project>/{FAILURE_MART_PATH}' contain the "
                "lone "
                "surrogate escape '\\ud83d' at line 5, column 32"
            ),
            expected_location=(5, 32),
            expected_help=(
                "Python strings hold code points, not UTF-16 surrogate pairs; write the character "
                "as one escape: \\U0001F600 instead of the surrogate pair, or write the character "
                "itself"
            ),
        ),
        failure_case(
            name="engine-error-macro-argument-bytes",
            expected_code="P001",
            files={
                _ENGINE_ERROR_MACROS: _ENGINE_ERROR_CENTS,
                **mart_body_files(
                    "SELECT customer_id, @py.cents(b'amount') AS cents\nFROM "
                    '__ref("stg_orders")\n'
                ),
            },
            expected_message=(
                f"Macro arguments of '@py.cents' in '<project>/{FAILURE_MART_PATH}' "
                "use a bytes literal at line 5, column 31"
            ),
            expected_location=(5, 31),
            expected_help=(
                "Pass text as a string without the b prefix, for example 'orders' instead of "
                "b'orders'"
            ),
        ),
        failure_case(
            name="engine-error-macro-argument-unhashable-key",
            expected_code="P001",
            files={
                _ENGINE_ERROR_MACROS: _ENGINE_ERROR_CENTS,
                **mart_body_files(
                    'SELECT customer_id, @py.cents({[1]: 2}) AS cents\nFROM __ref("stg_orders")\n'
                ),
            },
            expected_message=(
                f"Macro arguments of '@py.cents' in '<project>/{FAILURE_MART_PATH}' "
                "use a list or dict as a dict key at line 5, column 32"
            ),
            expected_location=(5, 32),
            expected_help=(
                "Use a string, number, True, False, None, or a tuple of them as a dict key"
            ),
        ),
        failure_case(
            name="engine-error-macro-argument-complex",
            expected_code="P001",
            files={
                _ENGINE_ERROR_MACROS: _ENGINE_ERROR_CENTS,
                **mart_body_files(
                    'SELECT customer_id, @py.cents(2j) AS cents\nFROM __ref("stg_orders")\n'
                ),
            },
            expected_message=(
                f"Macro arguments of '@py.cents' in '<project>/{FAILURE_MART_PATH}' "
                "use a complex number literal at line 5, column 31"
            ),
            expected_location=(5, 31),
            expected_help=(
                "Pass int or float numbers; give a complex value's real and imaginary parts as two "
                "arguments"
            ),
        ),
        failure_case(
            name="engine-error-macro-argument-ellipsis",
            expected_code="P001",
            files={
                _ENGINE_ERROR_MACROS: _ENGINE_ERROR_CENTS,
                **mart_body_files(
                    'SELECT customer_id, @py.cents(...) AS cents\nFROM __ref("stg_orders")\n'
                ),
            },
            expected_message=(
                f"Macro arguments of '@py.cents' in '<project>/{FAILURE_MART_PATH}' "
                "use '...' (Ellipsis) at line 5, column 31"
            ),
            expected_location=(5, 31),
            expected_help=("Pass None, or a string the macro understands, instead of '...'"),
        ),
        failure_case(
            name="engine-error-undecodable-environment-variable",
            expected_code="P001",
            files=mart_body_files(
                f"SELECT customer_id, '@@ENV:{FAILURE_UNDECODABLE_ENV_VAR}' AS region\n"
                'FROM __ref("stg_orders")\n'
            ),
            expected_message=(
                f"Environment variable '{FAILURE_UNDECODABLE_ENV_VAR}' is not valid UTF-8 text: "
                "byte 17 starts an invalid UTF-8 sequence"
            ),
            expected_help=(
                f"Set '{FAILURE_UNDECODABLE_ENV_VAR}' to UTF-8 text, since SQLBuild cannot send "
                "undecodable bytes to a warehouse; the value is not shown because it may be a "
                "secret"
            ),
        ),
        failure_case(
            name="engine-error-macro-output-lone-surrogate",
            expected_code="P001",
            files={
                _ENGINE_ERROR_MACROS: (
                    'def cents(expression: str) -> str:\n    """Convert to cents."""\n'
                    "    return expression + chr(0xDCFF)\n"
                ),
                **mart_body_files(
                    "SELECT customer_id, @py.cents('amount') AS cents\nFROM __ref(\"stg_orders\")\n"
                ),
            },
            expected_message=(
                f"Macro '@py.cents' in '<project>/{FAILURE_MART_PATH}' returned text with the lone "
                "surrogate '\\udcff', which is not valid Unicode"
            ),
            expected_help=(
                "Return valid Unicode text from the macro; lone surrogates usually come from "
                "bytes decoded with errors='surrogateescape'"
            ),
        ),
        failure_case(
            name="engine-error-week-date-cursor-start",
            expected_code="P001",
            files={
                FAILURE_MART_PATH: _MART_HEADER_START
                + "  materialized incremental,\n  incremental_strategy append,\n"
                + "  cursor order_ts,\n  cursor_type timestamp,\n  cursor_grain day,\n"
                + '  cursor_start "2024-W01-1T25:00",\n);\n\n'
                + 'SELECT customer_id, CURRENT_TIMESTAMP AS order_ts\nFROM __ref("stg_orders")\n'
            },
            expected_message=(
                "model 'customer_totals': cursor_start value '2024-W01-1T25:00' is not a valid "
                "ISO timestamp: hour must be in 0..23"
            ),
        ),
        failure_case(
            name="engine-error-non-ascii-replay-duration",
            expected_code="P001",
            files={
                FAILURE_MART_PATH: _MART_HEADER_START
                + "  materialized incremental,\n  incremental_strategy append,\n"
                + '  replay_on_change "bounded-\u0661d",\n);\n\n'
                + 'SELECT customer_id\nFROM __ref("stg_orders")\n'
            },
            expected_message=(
                "model 'customer_totals': replay_on_change '\u0661d' uses digits outside ASCII"
            ),
            expected_help=(
                "write replay_on_change with ASCII digits 0-9, add this to the MODEL header:\n"
                "            MODEL (\n              replay_on_change 'bounded-14d',\n"
                "              ...\n            );"
            ),
        ),
        failure_case(
            name="engine-error-oversized-retention",
            expected_code="P001",
            files={
                FAILURE_MART_PATH: _MART_HEADER_START
                + '  materialized table,\n  time_travel_retention "99999999999999999999d",\n);\n\n'
                + 'SELECT customer_id\nFROM __ref("stg_orders")\n'
            },
            expected_message=(
                "model 'customer_totals': time_travel_retention '99999999999999999999d' has a "
                "number larger than a 64-bit integer"
            ),
            expected_help=(
                "use a time_travel_retention whose days fit in 64 bits, add this to the MODEL "
                "header:\n            MODEL (\n              time_travel_retention '7d',\n"
                "              ...\n            );"
            ),
        ),
    )
