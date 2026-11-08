"""Minimal projects that fail SQL analysis, semantic, contract, or target validation."""

from scripts.compiler_differential._helpers.analysis_corpus.analysis_session import (
    analysis_session_failure_cases,
)
from scripts.compiler_differential._helpers.analysis_corpus.assembly import (
    assembly_failure_cases,
)
from scripts.compiler_differential._helpers.analysis_corpus.contracts import (
    contracts_failure_cases,
)
from scripts.compiler_differential._helpers.analysis_corpus.lineage import (
    lineage_failure_cases,
)
from scripts.compiler_differential._helpers.analysis_corpus.semantic_checks import (
    semantic_checks_failure_cases,
)
from scripts.compiler_differential._helpers.analysis_corpus.sql_test_glue import (
    sql_test_glue_failure_cases,
)
from scripts.compiler_differential._helpers.corpus.case_builder import (
    failure_case,
    mart_body_files,
    staging_files,
)
from scripts.compiler_differential.constants import (
    FAILURE_BASE_CONFIG,
    FAILURE_BASE_STAGING,
    FAILURE_CONFIG_PATH,
    FAILURE_MART_PATH,
    FAILURE_SOURCES_PATH,
)
from scripts.compiler_differential.models import FailureCase

_MART_HEADER: str = '"Order totals per customer",'
_ORDERS: str = '__ref("stg_orders")'
_SNOWFLAKE_CONFIG: str = (
    'name = "failure_corpus"\nadapter = "snowflake"\n\n'
    '[connection]\naccount = "example"\nuser = "builder"\n'
)
_SQLITE_CONFIG: str = (
    'name = "failure_corpus"\nadapter = "sqlite"\n\n[connection]\npath = "failure.sqlite"\n'
)
_POSTGRES_CONFIG: str = (
    'name = "failure_corpus"\nadapter = "postgres"\n\n'
    '[connection]\nhost = "localhost"\ndatabase = "failure"\nschema = "public"\n'
)
_FUNCTION_PATH: str = "functions/sql/scaled_amount.sql"
_SCALED_AMOUNT: str = (
    "FUNCTION (\n"
    '  description "Scale an amount by an integer factor",\n'
    "  arguments (p_amount DOUBLE, p_factor INTEGER),\n"
    "  returns DOUBLE,\n);\n\n"
    "p_amount * p_factor\n"
)
_HOOK_PATH: str = "hooks/sql/record_refresh.sql"
_EXPRESSION_AUDIT: str = 'AUDIT ();\n\nSELECT *\nFROM __ref("@model")\nWHERE NOT (@expression)\n'
_EXPRESSION_AUDIT_PATH: str = "models/marts/_sqlbuild/_audits/generic/expression_is_true.sql"
_TEST_PATH: str = "tests/unit/test_stg_orders.sql"


def _mart(body: str) -> dict[str, str]:
    return mart_body_files(body)


def _mart_header(*, extra: str, body: str | None = None) -> dict[str, str]:
    files: dict[str, str] = mart_body_files(
        body
        or 'SELECT customer_id, SUM(amount) AS total_amount\nFROM __ref("stg_orders")\n'
        "GROUP BY customer_id\n"
    )
    return {
        FAILURE_MART_PATH: files[FAILURE_MART_PATH].replace(
            _MART_HEADER, f"{_MART_HEADER}\n{extra}"
        )
    }


def analysis_failure_cases() -> tuple[FailureCase, ...]:
    """Return the analysis-stage cases, then every analysis lane's own cases."""

    return (
        *_binding_cases(),
        *_type_cases(),
        *_grouping_cases(),
        *_metadata_cases(),
        *_contract_cases(),
        *_target_cases(),
        *_syntax_cases(),
        *_ordering_cases(),
        *analysis_session_failure_cases(),
        *semantic_checks_failure_cases(),
        *contracts_failure_cases(),
        *lineage_failure_cases(),
        *sql_test_glue_failure_cases(),
        *assembly_failure_cases(),
    )


def _binding_cases() -> tuple[FailureCase, ...]:
    return (
        failure_case(
            name="analysis-ambiguous-column",
            expected_code="B003",
            files=_mart(
                "SELECT order_id\n"
                f"FROM {_ORDERS} a\nJOIN {_ORDERS} b ON a.customer_id = b.customer_id\n"
            ),
        ),
        failure_case(
            name="analysis-unknown-qualifier",
            expected_code="B004",
            expected_message="Unknown table or alias 'x' referenced by column 'order_id'",
            files=_mart(f"SELECT x.order_id\nFROM {_ORDERS} o\n"),
        ),
        failure_case(
            name="analysis-alias-column-list-width",
            expected_code="B005",
            files=_mart(
                "SELECT o.a AS first_column\n"
                f"FROM (SELECT order_id, customer_id FROM {_ORDERS}) AS o(a, b, c)\n"
            ),
        ),
        failure_case(
            name="analysis-unknown-function",
            expected_code="B101",
            files=_mart(
                f"SELECT customer_id, not_a_function(amount) AS total_amount\nFROM {_ORDERS}\n"
            ),
        ),
        failure_case(
            name="analysis-function-argument-count",
            expected_code="B102",
            files=_mart(f"SELECT customer_id, SUBSTRING(status) AS total_amount\nFROM {_ORDERS}\n"),
        ),
        failure_case(
            name="analysis-udf-argument-count",
            expected_code="B102",
            files={
                _FUNCTION_PATH: _SCALED_AMOUNT,
                **_mart(
                    'SELECT customer_id, __udf("scaled_amount")(amount) AS total_amount\n'
                    f"FROM {_ORDERS}\n"
                ),
            },
        ),
        failure_case(
            name="analysis-unsupported-truth-test",
            expected_code="B103",
            expected_message="'IS NOT TRUE' is not supported for dialect Snowflake",
            files={
                FAILURE_CONFIG_PATH: _SNOWFLAKE_CONFIG
                + '\n[defaults]\ndatabase = "ANALYTICS"\nschema = "ANALYTICS"\n',
                **_mart(
                    f"SELECT customer_id, amount AS total_amount\nFROM {_ORDERS}\n"
                    "WHERE (amount > 1) IS NOT TRUE\n"
                ),
            },
        ),
        failure_case(
            name="analysis-sqlite-temporal-cast",
            expected_code="B002",
            expected_warning_code="B104",
            expected_codes=("B002", "B104"),
            files={
                FAILURE_CONFIG_PATH: _SQLITE_CONFIG,
                **_mart(
                    "SELECT customer_id, CAST(amount AS TIMESTAMP) AS total_amount, missing\n"
                    f"FROM {_ORDERS}\n"
                ),
            },
        ),
    )


def _type_cases() -> tuple[FailureCase, ...]:
    return (
        failure_case(
            name="analysis-non-boolean-predicate",
            expected_code="B211",
            files=_mart(
                f"SELECT customer_id, amount AS total_amount\nFROM {_ORDERS}\nWHERE status\n"
            ),
        ),
        failure_case(
            name="analysis-arithmetic-operands",
            expected_code="B212",
            files=_mart(f"SELECT customer_id, status + 1 AS total_amount\nFROM {_ORDERS}\n"),
        ),
        failure_case(
            name="analysis-function-argument-type",
            expected_code="B213",
            files=_mart(f"SELECT customer_id, ABS(status) AS total_amount\nFROM {_ORDERS}\n"),
        ),
        failure_case(
            name="analysis-set-operation-branch-types",
            expected_code="B215",
            files=_mart(
                f"SELECT customer_id, amount FROM {_ORDERS}\n"
                f"UNION ALL\nSELECT customer_id, DATE '2026-01-01' FROM {_ORDERS}\n"
            ),
        ),
        failure_case(
            name="analysis-unsupported-cast",
            expected_code="B218",
            files=_mart(f"SELECT customer_id, DATE '2026-13-45' AS placed_on\nFROM {_ORDERS}\n"),
        ),
    )


def _grouping_cases() -> tuple[FailureCase, ...]:
    return (
        failure_case(
            name="analysis-aggregate-in-where",
            expected_code="B231",
            files=_mart(
                f"SELECT customer_id\nFROM {_ORDERS}\nWHERE SUM(amount) > 1\nGROUP BY customer_id\n"
            ),
        ),
        failure_case(
            name="analysis-window-without-over",
            expected_code="B232",
            files=_mart(f"SELECT customer_id, ROW_NUMBER() AS position\nFROM {_ORDERS}\n"),
        ),
        failure_case(
            name="analysis-duplicate-cte-name",
            expected_code="B233",
            files=_mart(
                f"WITH totals AS (SELECT customer_id FROM {_ORDERS}),\n"
                f"totals AS (SELECT customer_id FROM {_ORDERS})\n"
                "SELECT customer_id FROM totals\n"
            ),
        ),
        failure_case(
            name="analysis-non-constant-limit",
            expected_code="B234",
            files=_mart(f"SELECT customer_id\nFROM {_ORDERS}\nLIMIT customer_id\n"),
        ),
    )


def _metadata_cases() -> tuple[FailureCase, ...]:
    return (
        failure_case(
            name="analysis-unique-key-unknown-column",
            expected_code="B300",
            expected_message="unique_key references unknown column 'order_key'",
            files=_mart_header(extra="  materialized table,\n  unique_key [order_key],"),
        ),
        failure_case(
            name="analysis-relationships-unknown-field",
            expected_code="B300",
            expected_message="relationships target has no column 'customer_key'",
            files=_mart_header(
                extra="  columns (\n"
                '    customer_id (audits [relationships (to __ref("stg_orders"), '
                "field customer_key)]),\n  ),"
            ),
        ),
        failure_case(
            name="analysis-accepted-values-type",
            expected_code="B301",
            expected_message="accepted_values",
            files=_mart_header(
                extra="  columns (\n"
                "    customer_id (audits [accepted_values (values [DATE '2026-01-01'])]),\n  ),"
            ),
        ),
        failure_case(
            name="analysis-cursor-type-family",
            expected_code="B301",
            expected_message="does not match column 'customer_id' type INT",
            files={
                FAILURE_MART_PATH: (
                    'MODEL (\n  description "Order totals per customer",\n'
                    "  materialized incremental,\n  incremental_strategy delete_insert,\n"
                    "  unique_key [customer_id],\n"
                    "  cursor customer_id,\n  cursor_type timestamp,\n  cursor_grain day,\n"
                    "  cursor_inputs (\n    stg_orders customer_id,\n  ),\n);\n\n"
                    "SELECT customer_id, SUM(amount) AS total_amount\n"
                    f"FROM {_ORDERS}\nGROUP BY customer_id\n"
                )
            },
        ),
        failure_case(
            name="analysis-udf-argument-family",
            expected_code="B301",
            expected_message="Function 'scaled_amount' argument 'p_amount' expects DOUBLE",
            files={
                _FUNCTION_PATH: _SCALED_AMOUNT,
                **_mart(
                    'SELECT customer_id, __udf("scaled_amount")(status, 2) AS total_amount\n'
                    f"FROM {_ORDERS}\n"
                ),
            },
        ),
        failure_case(
            name="analysis-expression-audit-unknown-column",
            expected_code="B002",
            expected_message="Unknown column 'discount' in output",
            files={
                _EXPRESSION_AUDIT_PATH: _EXPRESSION_AUDIT,
                **_mart_header(extra='  audits [expression_is_true (expression "discount >= 0")],'),
            },
        ),
        failure_case(
            name="analysis-test-fixture-unknown-column",
            expected_code="B302",
            expected_message="SQL test '__source__raw_orders' names unknown column 'discount'",
            files={
                _TEST_PATH: (
                    "TEST();\n\nWITH\n__source__raw_orders AS (\n"
                    "  SELECT 1 AS order_id, 10 AS customer_id, CAST(5 AS DOUBLE) AS amount,"
                    " 'placed' AS status, 2 AS discount\n),\n"
                    "__expected__stg_orders AS (\n  SELECT 1 AS order_id\n)\nSELECT 1\n"
                )
            },
        ),
    )


def _contract_cases() -> tuple[FailureCase, ...]:
    contract_header: str = '  description "Staged orders",\n  contract enforced,\n'
    query: str = 'SELECT order_id, customer_id, amount, status\nFROM __source("raw_orders")\n'
    return (
        failure_case(
            name="contract-nullability",
            expected_code="K004",
            expected_message="column 'customer_id' is declared non-null but may be nullable",
            files=staging_files(
                f"MODEL (\n{contract_header}"
                "  columns (\n    order_id (type INTEGER),\n"
                "    customer_id (type INTEGER, nullable false),\n"
                "    amount (type DOUBLE),\n    status (type VARCHAR),\n  ),\n);\n\n"
                "SELECT order_id, CAST(NULL AS INTEGER) AS customer_id, amount, status\n"
                'FROM __source("raw_orders")\n'
            ),
        ),
        failure_case(
            name="contract-without-columns",
            expected_code="K006",
            expected_message="model 'stg_orders' has contract enforced but declares no columns",
            files=staging_files(f"MODEL (\n{contract_header});\n\n{query}"),
        ),
        failure_case(
            name="contract-enum-literal",
            expected_code="K013",
            expected_message=(
                "value 'held' in column 'status' of model 'stg_orders' is not a member of enum "
                "order_status"
            ),
            files={
                "models/staging/_enums/order_status.sql": (
                    'ENUM (name order_status, members (PLACED "placed", SHIPPED "shipped"));\n'
                ),
                **staging_files(
                    f"MODEL (\n{contract_header}"
                    "  columns (\n    order_id (type INTEGER),\n    customer_id (type INTEGER),\n"
                    "    amount (type DOUBLE),\n    status (type order_status),\n  ),\n);\n\n"
                    "SELECT order_id, customer_id, amount, 'held' AS status\n"
                    'FROM __source("raw_orders")\n'
                ),
            },
        ),
        failure_case(
            name="contract-immediate-promotion",
            expected_code="K011",
            expected_message="contract enforced requires staged table promotion",
            files={
                FAILURE_CONFIG_PATH: FAILURE_BASE_CONFIG
                + '\n[settings]\ntable_promotion_mode = "immediate"\n',
                **staging_files(
                    f"MODEL (\n{contract_header}  materialized table,\n"
                    "  columns (\n    order_id (type INTEGER),\n    customer_id (type INTEGER),\n"
                    "    amount (type DOUBLE),\n    status (type VARCHAR),\n  ),\n);\n\n"
                    f"{query}"
                ),
            },
        ),
        failure_case(
            name="contract-dynamic-columns-unproven",
            expected_code="K011",
            expected_message="dynamic column contract is not proven",
            files=_mart_header(
                extra="  materialized table,\n  columns (customer_id (type INTEGER)),\n"
                "  dynamic_columns (\n    status_amounts (\n      pivot_column status,\n"
                "      value_column amount,\n      aggregate MAX,\n      type DOUBLE\n    )\n  ),",
                body=f"PIVOT {_ORDERS}\nON status\nUSING MAX(amount)\nGROUP BY customer_id\n",
            ),
        ),
        failure_case(
            name="contract-dynamic-columns-unsupported-dialect",
            expected_code="K011",
            expected_message="does not support compiler-proven",
            files={
                FAILURE_CONFIG_PATH: _POSTGRES_CONFIG,
                **_mart_header(
                    extra="  materialized table,\n  columns (customer_id (type INTEGER)),\n"
                    "  dynamic_columns (\n    status_amounts (\n      pivot_column status,\n"
                    "      value_column amount,\n      aggregate MAX,\n      type DOUBLE\n"
                    "    )\n  ),",
                    body=(
                        "SELECT customer_id, MAX(amount) AS placed\n"
                        f"FROM {_ORDERS}\nGROUP BY customer_id\n"
                    ),
                ),
            },
        ),
    )


def _target_cases() -> tuple[FailureCase, ...]:
    return (
        failure_case(
            name="target-missing-database",
            expected_code="S101",
            expected_message="snowflake execution requires explicit target database",
            expected_help="my_database",
            files={FAILURE_CONFIG_PATH: _SNOWFLAKE_CONFIG + '\n[defaults]\nschema = "ANALYTICS"\n'},
        ),
        failure_case(
            name="target-managed-loader-collision",
            expected_code="S102",
            expected_message="Managed loader write collision",
            files={
                FAILURE_CONFIG_PATH: FAILURE_BASE_CONFIG
                + '\n[targets.dev]\nschema = "dev"\nloader_schema = "raw"\n'
                + '\n[targets.prod]\nschema = "prod"\nloader_schema = "raw"\n',
                "python/loaders/orders.py": (
                    "from sqlbuild.loaders import loader\n\n\n"
                    "@loader\n"
                    "def loaded_orders(ctx):\n"
                    '    """Load raw orders."""\n'
                    '    return [{"order_id": 1}]\n'
                ),
                "sources/loaded.yml": (
                    "sources:\n  - name: loaded_orders\n    description: Loaded orders.\n"
                    "    managed: true\n    write_strategy: table\n"
                    "    columns:\n      - name: order_id\n        type: INTEGER\n"
                ),
            },
        ),
        failure_case(
            name="target-missing-write-schema",
            expected_code="S103",
            expected_message="has no explicitly resolved physical write schema",
            files={FAILURE_CONFIG_PATH: _SNOWFLAKE_CONFIG + 'database = "ANALYTICS"\n'},
        ),
        failure_case(
            name="target-named-without-schema",
            expected_code="S104",
            expected_message="Named target 'dev' must explicitly set schema",
            files={
                FAILURE_CONFIG_PATH: _SNOWFLAKE_CONFIG.replace(
                    'adapter = "snowflake"\n', 'adapter = "snowflake"\ndefault_target = "dev"\n'
                )
                + 'database = "ANALYTICS"\n\n[targets.dev]\ndatabase = "ANALYTICS"\n'
            },
        ),
    )


def _syntax_cases() -> tuple[FailureCase, ...]:
    return (
        failure_case(
            name="syntax-hook",
            expected_code="P026",
            expected_message="customer_totals",
            files={
                FAILURE_CONFIG_PATH: FAILURE_BASE_CONFIG
                + "\n[settings]\nrequire_sql_analysis = true\n",
                _HOOK_PATH: ('HOOK (\n  description "Record a refresh"\n);\n\nSELECT FROM WHERE\n'),
                **_mart_header(extra='  post_hooks [sql("record_refresh")],'),
            },
        ),
        failure_case(
            name="syntax-inline-hook",
            expected_code="P026",
            expected_message="customer_totals",
            files={
                FAILURE_CONFIG_PATH: FAILURE_BASE_CONFIG
                + "\n[settings]\nrequire_sql_analysis = true\n",
                **_mart_header(extra='  post_hooks [inline_sql("SELECT FROM WHERE")],'),
            },
        ),
        failure_case(
            name="syntax-function",
            expected_code="P001",
            expected_message="scaled_amount",
            files={
                _FUNCTION_PATH: _SCALED_AMOUNT.replace(
                    "p_amount * p_factor", "p_amount * (p_factor"
                ),
                **_mart(
                    'SELECT customer_id, __udf("scaled_amount")(amount, 2) AS total_amount\n'
                    f"FROM {_ORDERS}\n"
                ),
            },
        ),
        failure_case(
            name="syntax-source-expression",
            expected_code="P001",
            expected_message="raw_orders",
            files={
                FAILURE_SOURCES_PATH: (
                    "sources:\n  - name: raw_orders\n    description: Orders feed.\n"
                    "    expression: >-\n"
                    "      (SELECT 1 AS order_id,, 10 AS customer_id,\n"
                    "      CAST(5 AS DOUBLE) AS amount, 'placed' AS status)\n"
                )
            },
        ),
        failure_case(
            name="analysis-rejected-opt-out-hides-findings",
            expected_code="P009",
            expected_message="`sql_analysis false` is not needed for model 'customer_totals'",
            files={
                FAILURE_CONFIG_PATH: FAILURE_BASE_CONFIG
                + "\n[settings]\nrequire_sql_analysis = true\n",
                **_mart_header(
                    extra="  sql_analysis false,",
                    body=(
                        "SELECT customer_id, SUM(refund) AS total_amount\n"
                        f"FROM {_ORDERS}\nGROUP BY customer_id\n"
                    ),
                ),
            },
        ),
        failure_case(
            name="analysis-opt-out-conflicts-with-legacy-key",
            expected_code="P003",
            expected_message="MODEL sql_analysis conflicts with legacy sql_validation",
            files=_mart_header(extra="  sql_analysis false,\n  sql_validation true,"),
        ),
    )


def _ordering_cases() -> tuple[FailureCase, ...]:
    return (
        failure_case(
            name="order-two-failing-models",
            expected_code="B002",
            expected_message="Unknown column 'missing_column' in stg_orders",
            expected_codes=("B002", "B002"),
            files={
                **staging_files(
                    FAILURE_BASE_STAGING.replace("amount, status", "amount, discount, status")
                ),
                "models/marts/order_counts.sql": (
                    'MODEL (\n  description "Order counts per customer",\n);\n\n'
                    "SELECT customer_id, COUNT(missing_column) AS order_count\n"
                    f"FROM {_ORDERS}\nGROUP BY customer_id\n"
                ),
            },
        ),
        failure_case(
            name="order-binding-metadata-contract",
            expected_code="B002",
            expected_codes=("B002", "B300", "K001"),
            files={
                **staging_files(
                    'MODEL (\n  description "Staged orders",\n  contract enforced,\n'
                    "  columns (\n    order_id (type INTEGER),\n    customer_id (type INTEGER),\n"
                    "    amount (type DOUBLE),\n    status (type VARCHAR),\n"
                    "    discount (type DOUBLE),\n  ),\n);\n\n"
                    'SELECT order_id, customer_id, amount, status\nFROM __source("raw_orders")\n'
                ),
                **_mart_header(
                    extra="  materialized table,\n  unique_key [order_key],",
                    body=(
                        "SELECT customer_id, SUM(refund) AS total_amount\n"
                        f"FROM {_ORDERS}\nGROUP BY customer_id\n"
                    ),
                ),
            },
        ),
        failure_case(
            name="order-poisoned-downstream-uses",
            expected_code="B002",
            expected_message="Unknown column 'refund' in raw_orders",
            expected_notes=(
                "1 downstream output uses were not type-checked because of this error",
            ),
            files={
                **staging_files(
                    FAILURE_BASE_STAGING.replace("amount, status", "amount, refund, status")
                ),
                FAILURE_MART_PATH: (
                    'MODEL (\n  description "Order totals per customer",\n);\n\n'
                    "SELECT customer_id, SUM(refund) AS total_refund\n"
                    f"FROM {_ORDERS}\nGROUP BY customer_id\n"
                ),
            },
        ),
        failure_case(
            name="order-contract-error-then-warning",
            expected_code="K005",
            expected_warning_code="K003",
            expected_codes=("K005", "K003"),
            files=staging_files(
                'MODEL (\n  description "Staged orders",\n  contract enforced,\n'
                "  columns (\n    order_id (type INTEGER),\n    note (type VARCHAR),\n"
                "    customer_id (type INTEGER),\n    amount (type DOUBLE),\n  ),\n);\n\n"
                "SELECT order_id, NULL AS note, customer_id, amount, status\n"
                'FROM __source("raw_orders")\n'
            ),
        ),
        failure_case(
            name="order-closest-column-help",
            expected_code="B002",
            expected_help="did you mean 'amount'?",
            expected_notes=("stg_orders has: amount, status, customer_id, order_id",),
            expected_location=(5, 25),
            files=_mart(
                "SELECT customer_id, SUM(amonut) AS total_amount\n"
                f"FROM {_ORDERS}\nGROUP BY customer_id\n"
            ),
        ),
    )
