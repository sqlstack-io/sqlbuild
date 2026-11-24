"""E2E coverage of Rules on a selected plan, which must agree with a whole-project compile."""

from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path

import pytest

from sqlbuild._native import rules_bulk_cache_path
from tests.e2e.src.sqlbuild.cli.commands.main.rules._test_types import (
    SelectedGraphRuleCase,
    SelectedProjectWideRuleCase,
    SelectedRuleCacheCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.rules.helpers import (
    compile_diagnostics,
    custom_rule_source,
    duckdb_rules_project_toml,
    rule_cache_hits,
    run_compile_cli,
    run_unevaluated_rules_cli,
    write_model_files,
)

_FINDING_LINE: re.Pattern[str] = re.compile(r"^(\S+):\d+:\d+ \[(X?SQBR\w+)\] ", re.MULTILINE)
_RETURNS_PATH: str = "sql/models/sales/mart/sales__mart__returns__finance.sql"
_REFUNDS_PATH: str = "sql/models/sales/mart/sales__mart__refunds__audit.sql"
_ORDERS_PATH: str = "sql/models/sales/mart/sales__mart__orders.sql"
_CUSTOMERS_PATH: str = "sql/models/sales/mart/sales__mart__customers.sql"
_CONSUMER_PATH: str = "sql/models/finance/staging/finance__stg_v__orders.sql"
_FILES: tuple[tuple[str, str], ...] = (
    (
        "sql/models/sales/intermediate/clean/sales__int_clean__orders.sql",
        "MODEL (description 'Clean orders.');\nSELECT 1 AS order_id\n",
    ),
    (_ORDERS_PATH, "MODEL (description 'Published orders.');\nSELECT 1 AS order_id\n"),
    (
        _CUSTOMERS_PATH,
        "MODEL (description 'Published customers.');\n"
        'SELECT o.order_id FROM __ref("sales__int_clean__orders") AS o\n',
    ),
    (
        "sql/models/sales/mart/sales__mart__products.sql",
        "MODEL (description 'Published products.');\nSELECT 1 AS order_id\n",
    ),
    (
        _RETURNS_PATH,
        "MODEL (description 'Returns for finance.');\n"
        'SELECT o.order_id FROM __ref("sales__int_clean__orders") AS o\n',
    ),
    (_REFUNDS_PATH, "MODEL (description 'Refunds for audit.');\nSELECT 1 AS order_id\n"),
    (
        _CONSUMER_PATH,
        "MODEL (description 'Finance view of sales orders.');\n"
        'SELECT r.order_id FROM __ref("sales__mart__returns__finance") AS r\n'
        'JOIN __ref("sales__mart__refunds__audit") AS f ON r.order_id = f.order_id\n'
        'JOIN __ref("sales__mart__orders") AS o ON r.order_id = o.order_id\n',
    ),
)
_PRODUCTS_PATH: str = "sql/models/sales/mart/sales__mart__products.sql"
_CACHE_FILES: tuple[tuple[str, str], ...] = (
    (_ORDERS_PATH, "MODEL (description 'Published orders.');\nSELECT 1 AS order_id\n"),
    (_CUSTOMERS_PATH, "MODEL (description 'Published customers.');\nSELECT 1 AS customer_id\n"),
    (_PRODUCTS_PATH, "MODEL (description 'Published products.');\nSELECT 1 AS product_id\n"),
)
_QUALIFIED_IGNORES: str = (
    "\n[[rules.rule_ignores]]\n"
    'rules = ["SQBRPROJECT107"]\n'
    f'paths = ["{_RETURNS_PATH}", "{_REFUNDS_PATH}"]\n'
    'reason = "Reviewed qualified variants."\n'
)

_ORDERS_BASE_PATH: str = "sql/models/orders_base.sql"
_PUBLISHED_VIEW_RULE: str = custom_rule_source(
    code="XSQBRPUB001",
    body=(
        '    if model.name != "orders_base":\n'
        "        return []\n"
        '    published: bool = any(item.name == "orders_published" for item in ctx.graph.dependents(model))\n'
        "    return [] if published else [ctx.finding(subject=model)]\n"
    ),
)


@pytest.mark.parametrize(
    "test_case",
    [
        SelectedProjectWideRuleCase(
            description="a slice whose own majority differs keeps the whole-project shape",
            rules_toml='select = ["SQBRPROJECT107"]\n' + _QUALIFIED_IGNORES,
            selection=("+finance__stg_v__orders",),
            expected_compile_findings=(),
            expected_plan_findings=(),
            expected_plan_exit=0,
        ),
        SelectedProjectWideRuleCase(
            description="findings on models outside the slice are left out",
            rules_toml='select = ["SQBRPROJECT107", "SQBRMODEL101"]\n',
            selection=("sales__mart__returns__finance",),
            expected_compile_findings=(
                ("SQBRMODEL101", _CONSUMER_PATH),
                ("SQBRMODEL101", _CUSTOMERS_PATH),
                ("SQBRMODEL101", _RETURNS_PATH),
                ("SQBRPROJECT107", _REFUNDS_PATH),
                ("SQBRPROJECT107", _RETURNS_PATH),
            ),
            expected_plan_findings=(
                ("SQBRMODEL101", _RETURNS_PATH),
                ("SQBRPROJECT107", _RETURNS_PATH),
            ),
            expected_plan_exit=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_selection_when_planning_then_project_wide_rules_match_whole_project_compile(
    test_case: SelectedProjectWideRuleCase,
    tmp_path: Path,
) -> None:
    write_model_files(
        project_dir=tmp_path,
        toml=duckdb_rules_project_toml(project_dir=tmp_path, rules_toml=test_case.rules_toml),
        files=_FILES,
    )

    compiled: subprocess.CompletedProcess[str] = run_compile_cli(tmp_path)
    plans: tuple[subprocess.CompletedProcess[str], ...] = tuple(
        run_unevaluated_rules_cli(tmp_path, "plan", "--select", *test_case.selection)
        for _ in ("cold", "warm")
    )

    compile_findings: tuple[tuple[str, str], ...] = tuple(
        sorted({(str(item["code"]), str(item["path"])) for item in compile_diagnostics(compiled)})
    )
    assert compile_findings == test_case.expected_compile_findings, compiled.stdout
    for planned in plans:
        plan_output: str = planned.stdout + planned.stderr
        plan_findings: tuple[tuple[str, str], ...] = tuple(
            sorted({(code, path) for path, code in _FINDING_LINE.findall(plan_output)})
        )
        assert plan_findings == test_case.expected_plan_findings, plan_output
        assert planned.returncode == test_case.expected_plan_exit, plan_output
        assert "Traceback" not in plan_output, plan_output


@pytest.mark.parametrize(
    "test_case",
    [
        SelectedRuleCacheCase(
            description="a selected plan keeps compile's model-rule cache for the next compile",
            rules_toml='select = ["SQBRPROJECT107", "SQBRMODEL101"]\n',
            selection=("sales__mart__orders",),
            edited_path=_PRODUCTS_PATH,
            expected_entries=(_CUSTOMERS_PATH, _ORDERS_PATH, _PRODUCTS_PATH),
            expected_recompile_cache_hits=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_compile_cache_when_planning_selection_then_compile_and_plan_share_model_rules(
    test_case: SelectedRuleCacheCase,
    tmp_path: Path,
) -> None:
    write_model_files(
        project_dir=tmp_path,
        toml=duckdb_rules_project_toml(project_dir=tmp_path, rules_toml=test_case.rules_toml),
        files=_CACHE_FILES,
    )
    native_cache: Path = rules_bulk_cache_path(project_dir=tmp_path, file_name="native.json")

    compiled: subprocess.CompletedProcess[str] = run_compile_cli(tmp_path)
    after_compile: dict[str, object] = json.loads(native_cache.read_text(encoding="utf-8"))
    planned: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "plan", "--select", *test_case.selection
    )
    after_plan: dict[str, object] = json.loads(native_cache.read_text(encoding="utf-8"))
    edited: Path = tmp_path / test_case.edited_path
    _ = edited.write_text(
        edited.read_text(encoding="utf-8").replace("SELECT 1", "SELECT 2"), encoding="utf-8"
    )
    recompiled: subprocess.CompletedProcess[str] = run_compile_cli(tmp_path)

    assert compiled.returncode == 0, compiled.stdout + compiled.stderr
    assert planned.returncode == 0, planned.stdout + planned.stderr
    assert recompiled.returncode == 0, recompiled.stdout + recompiled.stderr
    assert tuple(sorted(after_compile["entries"])) == test_case.expected_entries  # type: ignore[arg-type]
    assert after_plan == after_compile
    assert rule_cache_hits(recompiled) == test_case.expected_recompile_cache_hits, recompiled.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        SelectedGraphRuleCase(
            description="an unselected dependent still exempts its upstream model",
            view_sql='SELECT order_id FROM __ref("orders_base")\n',
            selection=("+orders_report",),
            expected_compile_findings=(),
            expected_plan_findings=(),
            expected_plan_exit=0,
        ),
        SelectedGraphRuleCase(
            description="a finding compile reports on a selected model is still reported",
            view_sql="SELECT 1 AS order_id\n",
            selection=("+orders_report",),
            expected_compile_findings=(("XSQBRPUB001", _ORDERS_BASE_PATH),),
            expected_plan_findings=(("XSQBRPUB001", _ORDERS_BASE_PATH),),
            expected_plan_exit=1,
        ),
        SelectedGraphRuleCase(
            description="a finding on an unselected model is left out",
            view_sql="SELECT 1 AS order_id\n",
            selection=("orders_published",),
            expected_compile_findings=(("XSQBRPUB001", _ORDERS_BASE_PATH),),
            expected_plan_findings=(),
            expected_plan_exit=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_selection_when_planning_then_graph_reading_model_rules_see_whole_project(
    test_case: SelectedGraphRuleCase,
    tmp_path: Path,
) -> None:
    write_model_files(
        project_dir=tmp_path,
        toml=duckdb_rules_project_toml(
            project_dir=tmp_path,
            rules_toml=(
                'select = ["XSQBRPUB001"]\n\n[rules.thresholds]\nmin_custom_rule_test_cases = 0\n'
            ),
        ),
        files=(
            ("rules/published.py", _PUBLISHED_VIEW_RULE),
            (_ORDERS_BASE_PATH, "MODEL (description 'Orders.');\nSELECT 1 AS order_id\n"),
            (
                "sql/models/orders_report.sql",
                "MODEL (description 'Order report.');\n"
                'SELECT order_id FROM __ref("orders_base")\n',
            ),
            (
                "sql/models/orders_published.sql",
                "MODEL (description 'Published orders.', materialized view);\n"
                + test_case.view_sql,
            ),
        ),
    )

    compiled: subprocess.CompletedProcess[str] = run_compile_cli(tmp_path)
    plans: tuple[subprocess.CompletedProcess[str], ...] = tuple(
        run_unevaluated_rules_cli(tmp_path, "plan", "--select", *test_case.selection)
        for _ in ("cold", "warm")
    )

    compile_findings: tuple[tuple[str, str], ...] = tuple(
        sorted({(str(item["code"]), str(item["path"])) for item in compile_diagnostics(compiled)})
    )
    assert compile_findings == test_case.expected_compile_findings, compiled.stdout
    for planned in plans:
        plan_output: str = planned.stdout + planned.stderr
        plan_findings: tuple[tuple[str, str], ...] = tuple(
            sorted({(code, path) for path, code in _FINDING_LINE.findall(plan_output)})
        )
        assert plan_findings == test_case.expected_plan_findings, plan_output
        assert planned.returncode == test_case.expected_plan_exit, plan_output
        assert "Traceback" not in plan_output, plan_output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
