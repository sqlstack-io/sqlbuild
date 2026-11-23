"""E2E coverage of the published-layer Rules on the orders and finance example."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.rules._test_types import PublishedLayerCase
from tests.e2e.src.sqlbuild.cli.commands.main.rules.helpers import (
    compile_diagnostics,
    run_compile_cli,
    write_model_files,
)

_PROJECT_TOML: str = (
    'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQBRGRAPH103", "SQBRPROJECT107"]\n'
)
_ALLOWLIST: str = (
    "\n[rules.rule_options.SQBRPROJECT107]\n"
    'allowed_qualifiers = [{ domain = "sales", qualifier = "finance", '
    'reason = "Finance reads a reviewed published variant." }]\n'
)
_FINANCE_PATH: str = "sql/models/sales/mart/sales__mart__orders__finance.sql"
_INTERMEDIATE: tuple[str, str] = (
    "sql/models/sales/intermediate/clean/sales__int_clean__orders.sql",
    "MODEL (description 'Clean orders.');\nSELECT 1 AS order_id\n",
)
_ORDERS: tuple[str, str] = (
    "sql/models/sales/mart/sales__mart__orders.sql",
    "MODEL (description 'Published orders.');\n"
    'SELECT o.order_id FROM __ref("sales__int_clean__orders") AS o\n',
)
_CUSTOMERS: tuple[str, str] = (
    "sql/models/sales/mart/sales__mart__customers.sql",
    "MODEL (description 'Published customers.');\n"
    'SELECT o.order_id FROM __ref("sales__int_clean__orders") AS o\n',
)
_FINANCE_VARIANT: tuple[str, str] = (
    _FINANCE_PATH,
    "MODEL (description 'Orders reshaped for finance.');\n"
    'SELECT o.order_id FROM __ref("sales__mart__orders") AS o\n',
)
_FINANCE_STAGING: tuple[str, str] = (
    "sql/models/finance/staging/finance__stg_v__orders.sql",
    "MODEL (description 'Finance view of orders.');\n"
    'SELECT o.order_id FROM __ref("sales__mart__orders__finance") AS o\n',
)
_ORDERS_VIEW: tuple[str, str] = (
    "sql/models/sales/mart/sales__mart_v__orders.sql",
    "MODEL (description 'Orders view.');\n"
    'SELECT o.order_id FROM __ref("sales__mart__orders") AS o\n',
)
_FINANCE_FROM_PUBLISHED: tuple[str, str] = (
    "sql/models/finance/staging/finance__stg_v__orders.sql",
    "MODEL (description 'Finance view of orders.');\n"
    'SELECT o.order_id FROM __ref("sales__mart__orders") AS o\n',
)
_COUNTRY_GLOB: str = (
    "\n[rules.rule_options.SQBRPROJECT107]\n"
    'allowed_qualifiers = [{ domain = "sales", qualifier = "[a-z][a-z][a-z]", '
    'reason = "Published once per country." }]\n'
)
_REGIONAL_PATHS: str = (
    "\n[rules.rule_options.SQBRPROJECT107]\n"
    'allowed_qualifiers = [{ paths = ["sql/models/sales/mart/regional/*"], '
    'reason = "Published once per country." }]\n'
)
_COUNTRIES: tuple[tuple[str, str], ...] = tuple(
    (
        f"sql/models/sales/mart/regional/sales__mart__orders__{country}.sql",
        f"MODEL (description 'Orders published for {country}.');\n"
        'SELECT o.order_id FROM __ref("sales__int_clean__orders") AS o\n',
    )
    for country in ("deu", "fra", "ita")
)
_FINANCE_FROM_INTERMEDIATE: tuple[str, str] = (
    _FINANCE_PATH,
    "MODEL (description 'Orders reshaped for finance.');\n"
    'SELECT o.order_id FROM __ref("sales__int_clean__orders") AS o\n',
)
_PER_COUNTRY: tuple[tuple[str, str], ...] = (_INTERMEDIATE, _ORDERS, _CUSTOMERS, *_COUNTRIES)
_ORDERS_FINANCE: tuple[tuple[str, str], ...] = (
    _INTERMEDIATE,
    _ORDERS,
    _CUSTOMERS,
    _FINANCE_VARIANT,
    _FINANCE_STAGING,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PublishedLayerCase(
            description="a consumer variant in the producing domain's published layer",
            files=_ORDERS_FINANCE,
            rules_toml="",
            expected_findings=(("SQBRGRAPH103", _FINANCE_PATH), ("SQBRPROJECT107", _FINANCE_PATH)),
            expected_exit=1,
        ),
        PublishedLayerCase(
            description="an allowlisted qualifier passes the shape rule but not the read rule",
            files=_ORDERS_FINANCE,
            rules_toml=_ALLOWLIST,
            expected_findings=(("SQBRGRAPH103", _FINANCE_PATH),),
            expected_exit=1,
        ),
        PublishedLayerCase(
            description="a per-country family without an entry outnumbers the unqualified models",
            files=_PER_COUNTRY,
            rules_toml="",
            expected_findings=(
                ("SQBRPROJECT107", "sql/models/sales/mart/sales__mart__customers.sql"),
                ("SQBRPROJECT107", "sql/models/sales/mart/sales__mart__orders.sql"),
            ),
            expected_exit=1,
        ),
        PublishedLayerCase(
            description="one qualifier glob replaces a literal entry per country",
            files=_PER_COUNTRY,
            rules_toml=_COUNTRY_GLOB,
            expected_findings=(),
            expected_exit=0,
        ),
        PublishedLayerCase(
            description="a paths entry leaves out the per-country folder",
            files=_PER_COUNTRY,
            rules_toml=_REGIONAL_PATHS,
            expected_findings=(),
            expected_exit=0,
        ),
        PublishedLayerCase(
            description="a qualifier the glob does not match is still flagged",
            files=(*_PER_COUNTRY, _FINANCE_FROM_INTERMEDIATE),
            rules_toml=_COUNTRY_GLOB,
            expected_findings=(("SQBRPROJECT107", _FINANCE_PATH),),
            expected_exit=1,
        ),
        PublishedLayerCase(
            description="shared logic in intermediate and a view of its own entity",
            files=(_INTERMEDIATE, _ORDERS, _CUSTOMERS, _ORDERS_VIEW, _FINANCE_FROM_PUBLISHED),
            rules_toml="",
            expected_findings=(),
            expected_exit=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_published_models_when_compiling_then_layer_rules_report_variants(
    test_case: PublishedLayerCase,
    tmp_path: Path,
) -> None:
    write_model_files(
        project_dir=tmp_path, toml=_PROJECT_TOML + test_case.rules_toml, files=test_case.files
    )

    result: subprocess.CompletedProcess[str] = run_compile_cli(tmp_path)

    findings: tuple[tuple[str, str], ...] = tuple(
        sorted((str(item["code"]), str(item["path"])) for item in compile_diagnostics(result))
    )
    assert findings == test_case.expected_findings, result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit, result.stderr


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
