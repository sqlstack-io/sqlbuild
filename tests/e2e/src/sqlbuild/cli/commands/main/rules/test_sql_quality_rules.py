"""E2E coverage of literal length, ranking caps, unused CTE outputs and ranking proofs."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import rules_bulk_cache_path
from tests.e2e.src.sqlbuild.cli.commands.main.rules._test_types import (
    ConstantLiteralCase,
    DuplicateLiteralCase,
    FixableReportingCase,
    RankingKeyProofCase,
    RankingSortDefaultCase,
    ReplayedFindingExitCase,
    SqlQualityRuleCase,
    UnparsedDialectRulesCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.rules.helpers import (
    diagnostic_codes,
    diagnostic_notes,
    finding_fixability,
    fixable_finding_count,
    help_fragment_presence,
    long_literal_hints,
    run_unevaluated_rules_cli,
    write_quality_project,
)

_PROJECT_TOML: str = (
    'name = "orders"\nadapter = "duckdb"\n\n[rules]\n'
    'select = ["SQLCLEAN018", "SQLCLEAN042", "SQLCLEAN043", "SQLCLEAN044"]\n\n'
    "[rules.thresholds]\nmax_literal_length = 30\nmax_ranking_order_by = 2\n"
)
_UPSTREAM_SQL: str = (
    "SELECT 1 AS order_id, 7 AS customer_id, 'open' AS status, 'n' AS note, 3 AS amount\n"
)
_SUMMARY_HEADER: str = 'MODEL (description "Order summary");\n'
_SHARED_LITERAL: str = "'this order is still waiting for payment'"
_OTHER_LITERAL: str = "'this ticket is still waiting for a reply'"
_RAW_HEADER: str = 'MODEL (description "Raw orders");\n'
_KEYED_RAW_HEADER: str = (
    'MODEL (description "Raw orders", columns (order_id (audits [unique, not_null])));\n'
)
_FIX_NOTE: str = "Fix available: `sqb format --fix` applies it after compiler verification"
_RENAMED_CHAIN_SQL: str = (
    "WITH orders AS (\n"
    "  SELECT r.order_id AS sale_id, r.customer_id, r.amount\n"
    '  FROM __ref("raw_orders") AS r\n'
    "),\n"
    "staged AS (\n"
    "  SELECT o.sale_id, o.customer_id, o.amount FROM orders AS o\n"
    "),\n"
    "final AS (\n"
    "  SELECT s.sale_id,"
    " ROW_NUMBER() OVER (PARTITION BY s.customer_id ORDER BY s.amount DESC, s.sale_id) AS rn\n"
    "  FROM staged AS s\n"
    ")\n"
    "SELECT f.sale_id, f.rn FROM final AS f\n"
)
_INCREMENTAL_HEADER_TEMPLATE: str = (
    "MODEL (\n"
    '  description "Raw orders",\n'
    "  materialized incremental,\n"
    "  incremental_strategy {strategy},\n"
    "  unique_key order_id,\n"
    "{cursor}"
    ");\n"
)
_CURSOR_CONFIG: str = (
    "  cursor order_date,\n"
    "  cursor_type timestamp,\n"
    "  cursor_grain day,\n"
    '  cursor_start "2026-01-01",\n'
)
_INCREMENTAL_SQL: str = (
    "SELECT 1 AS order_id, 7 AS customer_id, 3 AS amount,"
    " TIMESTAMP '2026-01-01 00:00:00' AS order_date\n"
)
_CURSOR_INCREMENTAL_SQL: str = (
    'SELECT order_id, customer_id, amount, order_date FROM __source("landing_order_events")\n'
)
_CURSOR_SOURCE_YML: str = (
    "sources:\n"
    "  - name: landing_order_events\n    description: Test source landing_order_events.\n"
    "    schema: landing\n"
    "    columns:\n"
    "      - name: order_id\n        type: INTEGER\n"
    "      - name: customer_id\n        type: INTEGER\n"
    "      - name: amount\n        type: INTEGER\n"
    "      - name: order_date\n        type: TIMESTAMP\n"
)
_SNAPSHOT_HEADER: str = (
    "MODEL (\n"
    '  description "Raw orders",\n'
    "  materialized snapshot,\n"
    "  unique_key [order_id],\n"
    "  snapshot_strategy timestamp,\n"
    "  updated_at order_date,\n"
    ");\n"
)
_KEYED_SOURCE_YML: str = (
    "sources:\n"
    "  - name: landing_orders\n    description: Test source landing_orders.\n"
    "    schema: landing\n"
    "    columns:\n"
    "      - name: order_id\n"
    "        type: INTEGER\n"
    "        audits: [unique, not_null]\n"
)


@pytest.mark.parametrize(
    "test_case",
    (
        SqlQualityRuleCase(
            description="every quality Rule reports and the unused-column fix applies",
            upstream_header='MODEL (description "Raw orders", columns (order_id (audits [unique, not_null])));\n',
            query_sql=(
                "WITH orders AS (\n"
                "  SELECT r.order_id, r.customer_id, r.status, r.note, r.amount\n"
                '  FROM __ref("raw_orders") AS r\n'
                "),\n"
                "final AS (\n"
                "  SELECT\n"
                "    o.order_id,\n"
                "    CASE WHEN o.status = 'open' THEN 'this order is still waiting for payment'"
                " ELSE 'done' END AS label,\n"
                "    ROW_NUMBER() OVER (PARTITION BY o.customer_id ORDER BY o.amount) AS amount_rank,\n"
                "    ROW_NUMBER() OVER (PARTITION BY o.customer_id"
                " ORDER BY o.amount, o.status, o.order_id) AS stable_rank\n"
                "  FROM orders AS o\n"
                ")\n"
                "SELECT f.order_id, f.label, f.amount_rank, f.stable_rank FROM final AS f\n"
            ),
            expected_findings=(
                ("SQLCLEAN042", 3),
                ("SQLCLEAN044", 9),
                ("SQLCLEAN018", 10),
                ("SQLCLEAN043", 11),
            ),
            expected_after_fix=(("SQLCLEAN044", 16), ("SQLCLEAN018", 19), ("SQLCLEAN043", 20)),
        ),
        SqlQualityRuleCase(
            description="a ranking order without a declared key cannot be proven",
            upstream_header='MODEL (description "Raw orders");\n',
            query_sql=(
                "WITH orders AS (\n"
                "  SELECT r.order_id, r.customer_id\n"
                '  FROM __ref("raw_orders") AS r\n'
                "),\n"
                "final AS (\n"
                "  SELECT o.order_id,"
                " ROW_NUMBER() OVER (PARTITION BY o.customer_id ORDER BY o.order_id) AS rn\n"
                "  FROM orders AS o\n"
                ")\n"
                "SELECT f.order_id, f.rn FROM final AS f\n"
            ),
            expected_findings=(("SQLCLEAN018", 7),),
            expected_after_fix=(("SQLCLEAN018", 12),),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_quality_rule_violations_when_compiling_and_fixing_then_findings_and_fixes_match(
    test_case: SqlQualityRuleCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(_PROJECT_TOML, encoding="utf-8")
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "raw_orders.sql").write_text(test_case.upstream_header + _UPSTREAM_SQL)
    (models / "order_summary.sql").write_text(_SUMMARY_HEADER + test_case.query_sql)

    compiled: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "compile", "--json", "--no-cache"
    )
    fixed: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(tmp_path, "format", "--fix")
    recompiled: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "compile", "--json", "--no-cache"
    )

    before: list[dict[str, Any]] = json.loads(compiled.stdout)["diagnostics"]
    after: list[dict[str, Any]] = json.loads(recompiled.stdout)["diagnostics"]
    assert compiled.returncode == 1, compiled.stdout + compiled.stderr
    assert tuple((item["code"], item["line"]) for item in before) == test_case.expected_findings
    assert fixed.returncode == 0, fixed.stdout + fixed.stderr
    assert tuple((item["code"], item["line"]) for item in after) == test_case.expected_after_fix


@pytest.mark.parametrize(
    "test_case",
    (
        RankingKeyProofCase(
            description="a unique column audit proves the order through a rename and a CTE chain",
            upstream_header=(
                'MODEL (description "Raw orders", columns (order_id (audits [unique, not_null])));\n'
            ),
            upstream_sql=_UPSTREAM_SQL,
            query_sql=_RENAMED_CHAIN_SQL,
            expected_codes=(),
        ),
        RankingKeyProofCase(
            description="a merge unique_key proves the order through a rename",
            upstream_header=_INCREMENTAL_HEADER_TEMPLATE.format(
                strategy="merge", cursor=_CURSOR_CONFIG
            ),
            upstream_sql=_CURSOR_INCREMENTAL_SQL,
            query_sql=_RENAMED_CHAIN_SQL,
            expected_codes=(),
            extra_files=(("sources/landing_events.yml", _CURSOR_SOURCE_YML),),
        ),
        RankingKeyProofCase(
            description="a key-matched delete_insert unique_key proves the order",
            upstream_header=_INCREMENTAL_HEADER_TEMPLATE.format(
                strategy="delete_insert", cursor=""
            ),
            upstream_sql=_INCREMENTAL_SQL,
            query_sql=_RENAMED_CHAIN_SQL,
            expected_codes=(),
        ),
        RankingKeyProofCase(
            description="a cursor-ranged delete_insert unique_key is not a key",
            upstream_header=_INCREMENTAL_HEADER_TEMPLATE.format(
                strategy="delete_insert", cursor=_CURSOR_CONFIG
            ),
            upstream_sql=_CURSOR_INCREMENTAL_SQL,
            query_sql=_RENAMED_CHAIN_SQL,
            expected_codes=("SQLCLEAN018",),
            extra_files=(("sources/landing_events.yml", _CURSOR_SOURCE_YML),),
        ),
        RankingKeyProofCase(
            description="a snapshot unique_key identifies entities, not versions",
            upstream_header=_SNAPSHOT_HEADER,
            upstream_sql=_INCREMENTAL_SQL,
            query_sql=_RENAMED_CHAIN_SQL,
            expected_codes=("SQLCLEAN018",),
        ),
        RankingKeyProofCase(
            description="a unique audit on a nullable column is not a key",
            upstream_header='MODEL (description "Raw orders", columns (order_id (audits [unique])));\n',
            upstream_sql=_UPSTREAM_SQL,
            query_sql=_RENAMED_CHAIN_SQL,
            expected_codes=("SQLCLEAN018",),
        ),
        RankingKeyProofCase(
            description="a unique audit on a nullable false column is a key",
            upstream_header=(
                'MODEL (description "Raw orders",'
                " columns (order_id (nullable false, audits [unique])));\n"
            ),
            upstream_sql=_UPSTREAM_SQL,
            query_sql=_RENAMED_CHAIN_SQL,
            expected_codes=(),
        ),
        RankingKeyProofCase(
            description="a declared source key proves the order of a source read",
            upstream_header=_RAW_HEADER,
            upstream_sql=_UPSTREAM_SQL,
            query_sql=_RENAMED_CHAIN_SQL.replace(
                '__ref("raw_orders")', '__source("landing_orders")'
            ),
            expected_codes=(),
            extra_files=(("sources/landing.yml", _KEYED_SOURCE_YML),),
        ),
        RankingKeyProofCase(
            description="a QUALIFY window cannot prove its own order",
            upstream_header=_KEYED_RAW_HEADER,
            upstream_sql=_UPSTREAM_SQL,
            query_sql=(
                "SELECT r.customer_id, r.order_id\n"
                'FROM __ref("raw_orders") AS r\n'
                "QUALIFY ROW_NUMBER() OVER (PARTITION BY r.customer_id ORDER BY r.amount DESC) = 1\n"
            ),
            expected_codes=("SQLCLEAN018",),
        ),
        RankingKeyProofCase(
            description="an upstream hashed row key with a unique audit is a tie-breaker",
            upstream_header=(
                'MODEL (description "Raw orders", columns (row_key (audits [unique, not_null])));\n'
            ),
            upstream_sql=("SELECT HASH(7, 11, 1) AS row_key, 7 AS customer_id, 3 AS amount\n"),
            query_sql=(
                "WITH orders AS (\n"
                "  SELECT r.row_key, r.customer_id, r.amount\n"
                '  FROM __ref("raw_orders") AS r\n'
                "),\n"
                "final AS (\n"
                "  SELECT o.row_key,"
                " ROW_NUMBER() OVER (PARTITION BY o.customer_id ORDER BY o.amount, o.row_key) AS rn\n"
                "  FROM orders AS o\n"
                ")\n"
                "SELECT f.row_key, f.rn FROM final AS f\n"
            ),
            expected_codes=(),
        ),
        RankingKeyProofCase(
            description="the renamed chain is unproven without a declared key",
            upstream_header='MODEL (description "Raw orders");\n',
            upstream_sql=_UPSTREAM_SQL,
            query_sql=_RENAMED_CHAIN_SQL,
            expected_codes=("SQLCLEAN018",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_declared_upstream_keys_when_compiling_then_ranking_proof_follows_them(
    test_case: RankingKeyProofCase, tmp_path: Path
) -> None:
    write_quality_project(
        project_dir=tmp_path,
        toml=_PROJECT_TOML,
        upstream=test_case.upstream_header + test_case.upstream_sql,
        summary=_SUMMARY_HEADER + test_case.query_sql,
    )
    for path, contents in test_case.extra_files:
        (tmp_path / path).parent.mkdir(parents=True, exist_ok=True)
        (tmp_path / path).write_text(contents, encoding="utf-8")

    compiled: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "compile", "--json", "--no-cache"
    )

    assert diagnostic_codes(compiled) == test_case.expected_codes, compiled.stdout + compiled.stderr


@pytest.mark.parametrize(
    "test_case",
    (
        RankingSortDefaultCase(
            description="five sort keys fit the default",
            order_by="o.amount, o.status, o.note, o.customer_id, o.order_id",
            expected_codes=(),
        ),
        RankingSortDefaultCase(
            description="six sort keys exceed the default",
            order_by="o.amount, o.status, o.note, o.customer_id, o.amount + 1, o.order_id",
            expected_codes=("SQLCLEAN043",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_default_ranking_limit_when_compiling_then_five_sort_keys_are_allowed(
    test_case: RankingSortDefaultCase, tmp_path: Path
) -> None:
    write_quality_project(
        project_dir=tmp_path,
        toml='name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQLCLEAN043"]\n',
        upstream=_RAW_HEADER + _UPSTREAM_SQL,
        summary=(
            _SUMMARY_HEADER + "SELECT o.order_id,"
            f" ROW_NUMBER() OVER (ORDER BY {test_case.order_by}) AS rn\n"
            'FROM __ref("raw_orders") AS o\n'
        ),
    )

    compiled: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "compile", "--json", "--no-cache"
    )

    assert diagnostic_codes(compiled) == test_case.expected_codes, compiled.stdout + compiled.stderr


@pytest.mark.parametrize(
    "test_case",
    (
        FixableReportingCase(
            description="only the unused-column finding reports a fix",
            summary_sql=(
                _SUMMARY_HEADER + "WITH orders AS (\n"
                "  SELECT r.order_id, r.note\n"
                '  FROM __ref("raw_orders") AS r\n'
                "),\n"
                "final AS (\n"
                "  SELECT o.order_id, 'this order is still waiting for payment' AS label,"
                " ROW_NUMBER() OVER (ORDER BY o.order_id, o.order_id + 1, o.order_id + 2) AS rn\n"
                "  FROM orders AS o\n"
                ")\n"
                "SELECT f.order_id, f.label, f.rn FROM final AS f\n"
            ),
            expected_fixability=(
                ("SQLCLEAN042", True),
                ("SQLCLEAN043", False),
                ("SQLCLEAN044", False),
            ),
            expected_notes=(
                ("SQLCLEAN042", (_FIX_NOTE,)),
                ("SQLCLEAN043", ()),
                ("SQLCLEAN044", ()),
            ),
            expected_help_fragments=(
                ("SQLCLEAN043", "(the current value is 2)"),
                ("SQLCLEAN043", "[rules.thresholds]\n            max_ranking_order_by = 3"),
                ("SQLCLEAN044", '@const("_status_pattern")'),
                ("SQLCLEAN044", "[rules.thresholds]\n            max_literal_length = 39"),
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_fixable_and_manual_findings_when_reporting_then_only_fixable_ones_say_so(
    test_case: FixableReportingCase, tmp_path: Path
) -> None:
    write_quality_project(
        project_dir=tmp_path,
        toml=_PROJECT_TOML,
        upstream=_KEYED_RAW_HEADER + _UPSTREAM_SQL,
        summary=test_case.summary_sql,
    )

    rules_json: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "rules", "--json", "run", "SQLCLEAN"
    )
    rules_text: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "rules", "run", "SQLCLEAN"
    )
    compiled: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "compile", "--json", "--no-cache"
    )

    fixability: dict[str, bool] = finding_fixability(rules_json)
    assert tuple((code, fixability.get(code)) for code, _ in test_case.expected_fixability) == (
        test_case.expected_fixability
    )
    assert rules_text.stdout.count(_FIX_NOTE) == fixable_finding_count(rules_json)
    assert diagnostic_notes(compiled) == dict(test_case.expected_notes)
    assert help_fragment_presence(compiled, test_case.expected_help_fragments) == (True,) * len(
        test_case.expected_help_fragments
    )


@pytest.mark.parametrize(
    "test_case",
    (
        ConstantLiteralCase(
            description="a model-private constant satisfies the literal limit",
            summary_sql=(
                "MODEL (\n"
                '  description "Order summary",\n'
                '  constants (_waiting_label "this order is still waiting for payment"),\n'
                ");\n"
                'SELECT o.order_id, @const("_waiting_label") AS label'
                ' FROM __ref("raw_orders") AS o\n'
            ),
            expected_exit=0,
            expected_codes=(),
        ),
        ConstantLiteralCase(
            description="the same value inline exceeds the literal limit",
            summary_sql=(
                _SUMMARY_HEADER
                + "SELECT o.order_id, 'this order is still waiting for payment' AS label"
                ' FROM __ref("raw_orders") AS o\n'
            ),
            expected_exit=1,
            expected_codes=("SQLCLEAN044",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_long_value_as_constant_or_literal_when_compiling_then_only_literal_is_reported(
    test_case: ConstantLiteralCase, tmp_path: Path
) -> None:
    write_quality_project(
        project_dir=tmp_path,
        toml=_PROJECT_TOML,
        upstream=_RAW_HEADER + _UPSTREAM_SQL,
        summary=test_case.summary_sql,
    )

    compiled: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "compile", "--json", "--no-cache"
    )

    assert compiled.returncode == test_case.expected_exit, compiled.stdout + compiled.stderr
    assert diagnostic_codes(compiled) == test_case.expected_codes


@pytest.mark.parametrize(
    "test_case",
    (
        DuplicateLiteralCase(
            description="two models in one folder share a literal",
            literals=(
                ("models/sales/orders.sql", _SHARED_LITERAL),
                ("models/sales/refunds.sql", _SHARED_LITERAL),
                ("models/support/tickets.sql", _OTHER_LITERAL),
            ),
            expected_hints=(
                (
                    "models/sales/orders.sql",
                    "The identical literal also appears in models/sales/refunds.sql; their "
                    "nearest common folder is `models/sales`, so declare it once in "
                    "`models/sales/_sqlbuild/_constants/` and reference it with `@const` in "
                    "each file.",
                ),
                (
                    "models/sales/refunds.sql",
                    "The identical literal also appears in models/sales/orders.sql; their "
                    "nearest common folder is `models/sales`, so declare it once in "
                    "`models/sales/_sqlbuild/_constants/` and reference it with `@const` in "
                    "each file.",
                ),
                ("models/support/tickets.sql", None),
            ),
        ),
        DuplicateLiteralCase(
            description="a subfolder use moves the declaration to the inherited role",
            literals=(
                ("models/sales/orders.sql", _SHARED_LITERAL),
                ("models/sales/east/refunds.sql", _SHARED_LITERAL),
            ),
            expected_hints=(
                (
                    "models/sales/orders.sql",
                    "The identical literal also appears in models/sales/east/refunds.sql; their "
                    "nearest common folder is `models/sales`, so declare it once in "
                    "`models/sales/_sqlbuild/constants/` and reference it with `@const` in "
                    "each file.",
                ),
            ),
        ),
        DuplicateLiteralCase(
            description="uses in different top folders need a project-wide constant",
            literals=(
                ("models/sales/orders.sql", _SHARED_LITERAL),
                ("models/support/tickets.sql", _SHARED_LITERAL),
            ),
            expected_hints=(
                (
                    "models/sales/orders.sql",
                    "The identical literal also appears in models/support/tickets.sql; no "
                    "folder below the project roots holds every use, so declare it once in "
                    "top-level `constants/` and reference it with `@const` in each file.",
                ),
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_repeated_long_literal_when_running_rules_then_help_names_the_shared_location(
    test_case: DuplicateLiteralCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(_PROJECT_TOML, encoding="utf-8")
    for path, literal in test_case.literals:
        model: Path = tmp_path / path
        model.parent.mkdir(parents=True, exist_ok=True)
        model.write_text(f"{_SUMMARY_HEADER}SELECT {literal} AS label\n", encoding="utf-8")

    result: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "rules", "--json", "run", "SQLCLEAN044"
    )

    hints: dict[str, str] = long_literal_hints(result)
    assert tuple((path, hints.get(path) or None) for path, _ in test_case.expected_hints) == (
        test_case.expected_hints
    ), result.stdout


@pytest.mark.parametrize(
    "test_case",
    (
        ReplayedFindingExitCase(
            description="a SQL rule finding fails every command",
            code="SQLCLEAN043",
            expected_exit=1,
        ),
        ReplayedFindingExitCase(
            description="a lint-unavailable warning fails no command",
            code="L007",
            expected_exit=0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_cached_sql_finding_when_running_commands_then_only_rule_findings_fail(
    test_case: ReplayedFindingExitCase, tmp_path: Path
) -> None:
    write_quality_project(
        project_dir=tmp_path,
        toml=(
            'name = "orders"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
            '[connection]\ndatabase = "plan.duckdb"\n\n[targets.dev]\nschema = "analytics"\n\n'
            '[rules]\nselect = ["SQLCLEAN043"]\n'
        ),
        upstream=_RAW_HEADER + _UPSTREAM_SQL,
        summary=(
            _SUMMARY_HEADER + "SELECT o.order_id, ROW_NUMBER() OVER (ORDER BY o.amount,"
            " o.status, o.note, o.customer_id, o.amount + 1, o.order_id) AS rn\n"
            'FROM __ref("raw_orders") AS o\n'
        ),
    )
    seeded: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "compile", "--json"
    )
    assert diagnostic_codes(seeded) == ("SQLCLEAN043",), seeded.stdout + seeded.stderr
    # Replay the cached finding under the case's code: the engine cannot emit L007 yet.
    cache: Path = rules_bulk_cache_path(project_dir=tmp_path, file_name="sql.json")
    stored: dict[str, Any] = json.loads(cache.read_text(encoding="utf-8"))
    for entry in stored["entries"].values():
        for finding in entry["findings"]:
            finding["code"] = test_case.code
    _ = cache.write_text(json.dumps(stored), encoding="utf-8")

    compiled: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "compile", "--json"
    )
    exits: tuple[int, ...] = (
        compiled.returncode,
        run_unevaluated_rules_cli(tmp_path, "rules", "run", "SQLCLEAN043").returncode,
        run_unevaluated_rules_cli(tmp_path, "plan").returncode,
        run_unevaluated_rules_cli(tmp_path, "build").returncode,
    )

    assert (diagnostic_codes(compiled), exits) == (
        (test_case.code,),
        (test_case.expected_exit,) * len(exits),
    ), compiled.stdout + compiled.stderr


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])


@pytest.mark.parametrize(
    "test_case",
    (
        UnparsedDialectRulesCase(
            description="a parsed dialect reports the built-in rule finding",
            adapter_toml='adapter = "duckdb"\n',
            expected_codes=("SQBRMODEL102",),
            expected_message="SELECT * is allowed only",
            expected_exit=1,
        ),
        UnparsedDialectRulesCase(
            description="an unparsed dialect warns once per model and fails nothing",
            adapter_toml=(
                'adapter = "bigquery"\ndefault_target = "dev"\n\n'
                '[targets.dev]\ndatabase = "warehouse"\nschema = "analytics"\n'
            ),
            expected_codes=("L007", "L007"),
            expected_message=(
                "SQLBuild cannot parse bigquery SQL for built-in rules yet,"
                " so these rules were not evaluated: SQBRMODEL102"
            ),
            expected_exit=0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_project_dialect_when_running_built_in_rules_then_unparsed_dialects_only_warn(
    test_case: UnparsedDialectRulesCase, tmp_path: Path
) -> None:
    write_quality_project(
        project_dir=tmp_path,
        toml=(
            'name = "orders"\n' + test_case.adapter_toml + '\n[rules]\nselect = ["SQBRMODEL102"]\n'
        ),
        upstream=_RAW_HEADER + _UPSTREAM_SQL,
        summary=_SUMMARY_HEADER + 'SELECT * FROM __ref("raw_orders")\n',
    )

    compiled: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "compile", "--json"
    )
    ran: subprocess.CompletedProcess[str] = run_unevaluated_rules_cli(
        tmp_path, "rules", "run", "SQBRMODEL102"
    )

    messages: list[str] = [
        str(item["message"]) for item in json.loads(compiled.stdout)["diagnostics"]
    ]
    assert (
        diagnostic_codes(compiled),
        all(test_case.expected_message in message for message in messages),
        (compiled.returncode, ran.returncode),
    ) == (
        test_case.expected_codes,
        True,
        (test_case.expected_exit, test_case.expected_exit),
    ), compiled.stdout + compiled.stderr + ran.stdout + ran.stderr
