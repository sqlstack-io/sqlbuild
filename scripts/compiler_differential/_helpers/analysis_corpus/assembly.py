"""Failure cases owned by the native project assembly and target validation lane."""

from scripts.compiler_differential._helpers.corpus.case_builder import failure_case
from scripts.compiler_differential.constants import (
    FAILURE_BASE_CONFIG,
    FAILURE_CONFIG_PATH,
    FAILURE_MART_PATH,
)
from scripts.compiler_differential.models import FailureCase

_TRINO_ADAPTER_PATH: str = "adapters/warehouse/duckdb_trino.py"
_TRINO_ADAPTER: str = (
    "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n\n\n"
    "class DuckDbTrinoAdapter(DuckDbAdapter):\n"
    "    adapter_name = 'duckdb_trino'\n"
    "    sql_analysis_dialect_name = 'trino'\n"
)
_QUALIFY_HOOK_MART: str = (
    "MODEL (\n"
    '  description "Customer totals",\n'
    '  post_hooks [inline_sql("CREATE TABLE audit_copy AS SELECT 1 AS a, 2 AS b '
    'FROM (SELECT 1) AS t QUALIFY row_number() OVER (PARTITION BY a ORDER BY b) = 1")]\n'
    ");\n\n"
    'SELECT customer_id, SUM(amount) AS total_amount\nFROM __ref("stg_orders")\n'
    "GROUP BY customer_id\n"
)


def assembly_failure_cases() -> tuple[FailureCase, ...]:
    """Return this lane's failure cases; the lane appends here without editing shared lists."""

    return (
        failure_case(
            name="assembly-hook-syntax-unbuilt-dialect",
            expected_code="P026",
            expected_message="trino",
            files={
                FAILURE_CONFIG_PATH: FAILURE_BASE_CONFIG.replace(
                    'adapter = "duckdb"', 'adapter = "duckdb_trino"'
                )
                + "\n[settings]\nrequire_sql_analysis = true\n",
                _TRINO_ADAPTER_PATH: _TRINO_ADAPTER,
                FAILURE_MART_PATH: _QUALIFY_HOOK_MART,
            },
        ),
    )
