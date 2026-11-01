"""E2E coverage of configuration-only Rule exceptions, including relation-level exceptions."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.rules._test_types import RuleExceptionConfigCase
from tests.e2e.src.sqlbuild.cli.commands.main.rules.helpers import (
    diagnostic_messages,
    diagnostic_text,
    run_compile_cli,
    write_model_files,
)

_PROJECT_TOML: str = 'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQLCLEAN038"]\n'
_MODEL_PATH: str = "models/order_checks.sql"
_CUSTOMERS: str = (
    "CROSS JOIN with `customers` in CTE `order_checks` creates an explicit cartesian product"
)
_REGIONS: str = "CROSS JOIN with `regions` in CTE `final` creates an explicit cartesian product"
_EXCEPTION_HEAD: str = (
    f'\n[[rules.rule_exceptions]]\nrule = "SQLCLEAN038"\npath = "{_MODEL_PATH}"\n'
)
_REASON: str = 'reason = "Every order is checked for every customer and region."\n'
_CUSTOMERS_EXCEPTION: str = f'{_EXCEPTION_HEAD}relation = "customers"\n{_REASON}'
_REGIONS_EXCEPTION: str = f'{_EXCEPTION_HEAD}relation = "regions"\n{_REASON}'
_SHIPMENTS_EXCEPTION: str = f'{_EXCEPTION_HEAD}relation = "shipments"\n{_REASON}'
_CTE_EXCEPTIONS: str = (
    f'{_EXCEPTION_HEAD}relation = "customers"\ncte = "order_checks"\n{_REASON}'
    f'{_EXCEPTION_HEAD}relation = "regions"\ncte = "final"\n{_REASON}'
)
_MODEL_SQL: str = (
    "MODEL (description 'Order checks.');\n"
    "WITH order_checks AS (\n"
    '  SELECT o.order_id, c.customer_id FROM __ref("orders") AS o\n'
    "{directive}"
    '  CROSS JOIN __ref("customers") AS c\n'
    "),\n"
    "final AS (\n"
    "  SELECT k.order_id, k.customer_id, r.region_id FROM order_checks AS k\n"
    '  CROSS JOIN __ref("regions") AS r\n'
    ")\n"
    "SELECT f.order_id, f.customer_id, f.region_id FROM final AS f\n"
)
_PLAIN_SQL: str = _MODEL_SQL.format(directive="")
_DIRECTIVE_SQL: str = _MODEL_SQL.format(
    directive="  -- sqb: ignore SQLCLEAN038 because every order meets every customer\n"
)
_LITERAL_SQL: str = _PLAIN_SQL.replace(
    "SELECT f.order_id, f.customer_id, f.region_id FROM final AS f",
    "SELECT f.order_id, f.customer_id, f.region_id, 'sqb: ignore SQLCLEAN038' AS note FROM final AS f",
)
_TRAILING_SQL: str = _PLAIN_SQL.replace(
    '  CROSS JOIN __ref("customers") AS c\n',
    '  CROSS JOIN __ref("customers") AS c -- sqb: ignore SQLCLEAN038 because deliberate\n',
)
_UNNAMED_SQL: str = (
    "MODEL (description 'Order checks.');\n"
    'SELECT o.order_id, u.n FROM __ref("orders") AS o CROSS JOIN UNNEST([1, 2]) AS u(n)\n'
)
_UPSTREAM_FILES: tuple[tuple[str, str], ...] = (
    ("models/orders.sql", "MODEL (description 'Sample orders.');\nSELECT 1 AS order_id\n"),
    (
        "models/customers.sql",
        "MODEL (description 'Sample customers.');\nSELECT 1 AS customer_id\n",
    ),
    ("models/regions.sql", "MODEL (description 'Sample regions.');\nSELECT 1 AS region_id\n"),
)


@pytest.mark.parametrize(
    "test_case",
    [
        RuleExceptionConfigCase(
            description="no exception reports both joins",
            rules_toml="",
            model_sql=_PLAIN_SQL,
            expected_exit=1,
            expected_messages=(_CUSTOMERS, _REGIONS),
        ),
        RuleExceptionConfigCase(
            description="relation-level exception covers exactly one join",
            rules_toml=_CUSTOMERS_EXCEPTION,
            model_sql=_PLAIN_SQL,
            expected_exit=1,
            expected_messages=(_REGIONS,),
        ),
        RuleExceptionConfigCase(
            description="CTE-scoped exceptions cover both joins",
            rules_toml=_CTE_EXCEPTIONS,
            model_sql=_PLAIN_SQL,
            expected_exit=0,
        ),
        RuleExceptionConfigCase(
            description="stale relation-level exception fails",
            rules_toml=_CUSTOMERS_EXCEPTION + _REGIONS_EXCEPTION + _SHIPMENTS_EXCEPTION,
            model_sql=_PLAIN_SQL,
            expected_exit=1,
            expected_fragments=(
                "stale rule exception suppresses no finding: SQLCLEAN038 at "
                "models/order_checks.sql (relation shipments)",
            ),
        ),
        RuleExceptionConfigCase(
            description="inline directive is an error that prints its exception",
            rules_toml="",
            model_sql=_DIRECTIVE_SQL,
            expected_exit=1,
            expected_messages=(_CUSTOMERS, _REGIONS),
            expected_fragments=(
                "Inline Rule suppressions are not supported",
                '[[rules.rule_exceptions]]\n            rule = "SQLCLEAN038"\n'
                '            path = "models/order_checks.sql"\n'
                '            relation = "customers"\n'
                '            reason = "every order meets every customer"',
            ),
        ),
        RuleExceptionConfigCase(
            description="a string literal mentioning a directive compiles cleanly",
            rules_toml=_CTE_EXCEPTIONS,
            model_sql=_LITERAL_SQL,
            expected_exit=0,
        ),
        RuleExceptionConfigCase(
            description="a trailing directive after code is an error",
            rules_toml=_CTE_EXCEPTIONS,
            model_sql=_TRAILING_SQL,
            expected_exit=1,
            expected_fragments=(
                "Inline Rule suppressions are not supported",
                '            relation = "customers"\n            reason = "deliberate"',
            ),
        ),
        RuleExceptionConfigCase(
            description="an unnamed join prints a file-wide exception for its file",
            rules_toml="",
            model_sql=_UNNAMED_SQL,
            expected_exit=1,
            expected_messages=(
                "CROSS JOIN creates an explicit cartesian product with an unnamed relation; "
                "only a file-wide exception (rule and path) can cover it",
            ),
            expected_fragments=(
                '[[rules.rule_exceptions]]\n            rule = "SQLCLEAN038"\n'
                '            path = "models/order_checks.sql"\n',
            ),
        ),
        RuleExceptionConfigCase(
            description="removed allow_exceptions setting fails with migration help",
            rules_toml="allow_exceptions = false\n",
            model_sql=_PLAIN_SQL,
            expected_exit=1,
            expected_fragments=(
                "sets [rules] allow_exceptions, which SQLBuild no longer supports",
                "remove the allow_exceptions line from [rules]",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rule_exception_config_when_compiling_then_only_named_findings_are_excepted(
    test_case: RuleExceptionConfigCase,
    tmp_path: Path,
) -> None:
    write_model_files(
        project_dir=tmp_path,
        toml=_PROJECT_TOML + test_case.rules_toml,
        files=(*_UPSTREAM_FILES, (_MODEL_PATH, test_case.model_sql)),
    )

    result: subprocess.CompletedProcess[str] = run_compile_cli(tmp_path)

    text: str = diagnostic_text(result)
    assert result.returncode == test_case.expected_exit, text
    assert diagnostic_messages(result, "SQLCLEAN038") == test_case.expected_messages, text
    for fragment in test_case.expected_fragments:
        assert fragment in text


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
