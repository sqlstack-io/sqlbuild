"""E2E tests for sqb diff command."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import cast

import duckdb
import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.diff._test_types import (
    DiffCommandE2ETestCase,
    DiffJsonOutputE2ETestCase,
    DiffKeyFailureE2ETestCase,
    DiffSamplingPrecedenceE2ETestCase,
    DiffSamplingSeedE2ETestCase,
    QueryDiffOutcomeE2ETestCase,
    SingleDiffE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.diff.helpers import (
    build_both_environments,
    changed_key_examples,
    execute_duckdb,
    prepare_diff_project,
    reject_nonfinite_json_constant,
    run_sampled_diff,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        DiffCommandE2ETestCase(
            description="identical full diff returns zero",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "orders_snapshot",
                "No schema differences.",
                "No changed columns.",
            ),
        ),
        DiffCommandE2ETestCase(
            description="deterministic sample reports non exhaustive comparison scope",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--sample-rows",
                "2",
                "--sample-seed",
                "7",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "sampled 2 of 3 union keys (66.67%; seed 7)",
                "Sampled Rows",
                "No changed columns in sampled keys.",
                "prod     3  <not bounded by cursor>",
                "dev      3  <not bounded by cursor>",
            ),
        ),
        DiffCommandE2ETestCase(
            description="sample limit above population reports exhaustive comparison",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--sample-rows",
                "10",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "exhaustive (3 union keys; configured sample limit 10)",
                "Rows",
                "No changed columns.",
            ),
        ),
        DiffCommandE2ETestCase(
            description="non positive cli sample limit fails clearly",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--sample-rows",
                "0",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=1,
            expected_stderr_fragments=("diff --sample-rows must be positive",),
        ),
        DiffCommandE2ETestCase(
            description="conflicting sample and exhaustive overrides fail clearly",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--sample-rows",
                "2",
                "--exhaustive",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=1,
            expected_stderr_fragments=("diff --exhaustive cannot be combined with --sample-rows",),
        ),
        DiffCommandE2ETestCase(
            description="model safety limit fails before comparison",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--max-models",
                "1",
                "--select",
                "orders_snapshot",
                "customer_totals",
            ),
            expected_exit_code=1,
            expected_stderr_fragments=("diff selected 2 models, exceeding --max-models 1",),
        ),
        DiffCommandE2ETestCase(
            description="column safety limit fails before row comparison",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--max-columns",
                "4",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "model 'orders_snapshot' has 5 columns, exceeding --max-columns 4",
            ),
        ),
        DiffCommandE2ETestCase(
            description="partial cursor coverage warns and continues with union sample",
            mutation_sql=("DELETE FROM dev.orders_snapshot WHERE order_id = 1",),
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--bounded",
                "10000d",
                "--sample-rows",
                "2",
                "--sample-seed",
                "7",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "sampled 2 of 3 union keys (66.67%; seed 7)",
                "Warning: cursor coverage differs between sides",
                "comparison continued without narrowing the requested bounds",
                "prod only",
                "order_id=1",
            ),
        ),
        DiffCommandE2ETestCase(
            description="row mismatch returns nonzero and reports column mismatch",
            mutation_sql=(
                "UPDATE dev.orders_snapshot SET amount_cents = amount_cents + 5 WHERE order_id = 1",
            ),
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "joined: 3",
                "unequal",
                "amount_cents",
                "mismatches=1",
                "order_id=1 | 100 -> 105",
            ),
        ),
        DiffCommandE2ETestCase(
            description="concise side only samples are shown by default",
            mutation_sql=(
                "DELETE FROM dev.orders_sparse WHERE order_id = 1",
                "INSERT INTO dev.orders_sparse (order_id, customer_id) VALUES (99, 9)",
            ),
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--select",
                "orders_sparse",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "prod only",
                "order_id=1",
                "dev only",
                "order_id=99",
            ),
        ),
        DiffCommandE2ETestCase(
            description="concise multi column examples show multiple changed columns",
            mutation_sql=(
                (
                    "UPDATE dev.orders_snapshot SET customer_id = 99, "
                    "amount_cents = amount_cents + 5 WHERE order_id = 1"
                ),
            ),
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "customer_id",
                "amount_cents",
                "order_id=1 | 1 -> 99",
                "order_id=1 | 100 -> 105",
            ),
        ),
        DiffCommandE2ETestCase(
            description="verbose diff shows example row changes",
            mutation_sql=(
                "UPDATE dev.orders_snapshot SET amount_cents = amount_cents + 5 WHERE order_id = 1",
            ),
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--verbose",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "Examples",
                "order_id=1 | 100 -> 105",
            ),
        ),
        DiffCommandE2ETestCase(
            description="verbose diff shows side only key samples",
            mutation_sql=(
                "DELETE FROM dev.orders_sparse WHERE order_id = 1",
                "INSERT INTO dev.orders_sparse (order_id, customer_id) VALUES (99, 9)",
            ),
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--verbose",
                "--select",
                "orders_sparse",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "prod only",
                "order_id=1",
                "dev only",
                "order_id=99",
            ),
        ),
        DiffCommandE2ETestCase(
            description="excluded column changes are ignored",
            mutation_sql=(
                "UPDATE dev.orders_snapshot SET status = 'surprising' WHERE order_id = 1",
            ),
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=("No changed columns.",),
        ),
        DiffCommandE2ETestCase(
            description="tolerance pass stays clean",
            mutation_sql=(
                "UPDATE dev.orders_snapshot SET amount_cents = amount_cents + 1 WHERE order_id = 1",
            ),
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=("No changed columns.",),
        ),
        DiffCommandE2ETestCase(
            description="tolerance fail reports mismatch",
            mutation_sql=(
                "UPDATE dev.orders_snapshot SET amount_cents = amount_cents + 2 WHERE order_id = 1",
            ),
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "amount_cents",
                "mismatches=1",
                "order_id=1 | 100 -> 102",
            ),
        ),
        DiffCommandE2ETestCase(
            description="multi model output uses global header and divider",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--select",
                "orders_snapshot",
                "customer_totals",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "selected models: 2",
                "orders_snapshot",
                "customer_totals",
                "────────────────",
            ),
        ),
        DiffCommandE2ETestCase(
            description="example caps can be overridden",
            mutation_sql=(
                "UPDATE dev.orders_snapshot SET amount_cents = amount_cents + 5 WHERE order_id = 1",
                "UPDATE dev.orders_snapshot SET amount_cents = amount_cents + 7 WHERE order_id = 2",
            ),
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--max-column-examples",
                "2",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "order_id=1 | 100 -> 105",
                "order_id=2 | 200 -> 207",
            ),
        ),
        DiffCommandE2ETestCase(
            description="schema only ignores row drift",
            mutation_sql=(
                "UPDATE dev.orders_snapshot SET amount_cents = amount_cents + 5 WHERE order_id = 1",
            ),
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--schema-only",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=("No schema differences.", "order_id"),
        ),
        DiffCommandE2ETestCase(
            description="bounded fallback on cursorless model succeeds",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--bounded",
                "30d",
                "--select",
                "customer_totals",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Fallback",
                "no cursor configured; used full row diff",
                "No changed columns.",
            ),
        ),
        DiffCommandE2ETestCase(
            description="missing unique key fails clearly",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--select",
                "daily_revenue",
            ),
            expected_exit_code=1,
            expected_stderr_fragments=("model 'daily_revenue' requires unique_key for row diff",),
        ),
        DiffCommandE2ETestCase(
            description="an unknown cli key column fails with a coded error",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--key",
                "nope",
                "--select",
                "daily_revenue",
            ),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[X311]",
                "row diff key column 'nope' does not exist in prod.daily_revenue",
            ),
        ),
        DiffCommandE2ETestCase(
            description="a relation missing on the FROM side fails with a coded error",
            mutation_sql=("DROP TABLE prod.daily_revenue",),
            command=("--no-color", "diff", "prod:dev", "--full", "--select", "daily_revenue"),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[X310]",
                "model 'daily_revenue' has no relation prod.daily_revenue in the FROM target",
            ),
        ),
        DiffCommandE2ETestCase(
            description="a relation missing on the TO side fails with a coded error",
            mutation_sql=("DROP TABLE dev.daily_revenue",),
            command=("--no-color", "diff", "prod:dev", "--full", "--select", "daily_revenue"),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "error[X310]",
                "model 'daily_revenue' has no relation dev.daily_revenue in the TO target",
            ),
        ),
        DiffCommandE2ETestCase(
            description="cli key enables model without declared unique key",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--key",
                "revenue_date",
                "--select",
                "daily_revenue",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=("revenue_date", "No changed columns."),
        ),
        DiffCommandE2ETestCase(
            description="explicit unkeyed mode compares model row multiplicities",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--unkeyed",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=("<not used>", "multiset rows"),
        ),
        DiffCommandE2ETestCase(
            description="environment schemas auto-create through real cli build flow",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=("orders_snapshot",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_diff_project_when_running_diff_then_behavior_matches_expected(
    test_case: DiffCommandE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_diff_project(tmp_path)
    build_both_environments(project_dir=project_dir)
    db_path: Path = project_dir / "diff.duckdb"

    mutation_sql: str
    for mutation_sql in test_case.mutation_sql:
        execute_duckdb(db_path=db_path, sql=mutation_sql)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    fragment: str
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout, result.stdout + result.stderr
    for fragment in test_case.expected_stderr_fragments:
        assert fragment in result.stderr, result.stdout + result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        DiffCommandE2ETestCase(
            description="inline keyed queries report changed values",
            mutation_sql=("UPDATE dev.orders_snapshot SET amount_cents = 999 WHERE order_id = 1",),
            command=(
                "--no-color",
                "diff",
                "--left-query",
                "SELECT order_id, amount_cents FROM prod.orders_snapshot",
                "--right-query",
                "SELECT order_id, amount_cents FROM dev.orders_snapshot",
                "--key",
                "order_id",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "selected query pairs: 1",
                "order_id=1 | 100 -> 999",
            ),
            expected_stderr_fragments=(
                "Query diff materialization  OK",
                "Query diff immediate cleanup  OK",
            ),
        ),
        DiffCommandE2ETestCase(
            description="inline keyed queries support deterministic sampling",
            mutation_sql=("UPDATE dev.orders_snapshot SET amount_cents = amount_cents + 10",),
            command=(
                "--no-color",
                "diff",
                "--left-query",
                "SELECT order_id, amount_cents FROM prod.orders_snapshot",
                "--right-query",
                "SELECT order_id, amount_cents FROM dev.orders_snapshot",
                "--key",
                "order_id",
                "--sample-rows",
                "2",
                "--sample-seed",
                "7",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=("sampled 2 of 3 union keys", "seed 7"),
        ),
        DiffCommandE2ETestCase(
            description="unkeyed queries preserve duplicate multiplicity",
            command=(
                "--no-color",
                "diff",
                "--left-query",
                "SELECT * FROM (VALUES (1), (1)) AS orders(order_id)",
                "--right-query",
                "SELECT * FROM (VALUES (1)) AS orders(order_id)",
                "--unkeyed",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=("left query only", "1"),
        ),
        DiffCommandE2ETestCase(
            description="unkeyed queries reserve internal multiplicity aliases safely",
            command=(
                "--no-color",
                "diff",
                "--left-query",
                "SELECT 0 AS __sqlbuild_count",
                "--right-query",
                "SELECT CAST(NULL AS INTEGER) AS __sqlbuild_count WHERE FALSE",
                "--unkeyed",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=("left query only", "1"),
        ),
        DiffCommandE2ETestCase(
            description="schema drift reports findings without invalid row comparison",
            command=(
                "--no-color",
                "diff",
                "--left-query",
                "SELECT 1 AS order_id",
                "--right-query",
                "SELECT 1 AS order_id, 100 AS amount_cents",
                "--key",
                "order_id",
            ),
            expected_exit_code=2,
            expected_stdout_fragments=(
                "outcome: incomplete",
                "schema differences: 1",
                "added columns: 1",
            ),
        ),
        DiffCommandE2ETestCase(
            description="query tolerance suppresses numeric differences",
            mutation_sql=(
                "UPDATE dev.orders_snapshot SET amount_cents = amount_cents + 5 WHERE order_id = 1",
            ),
            command=(
                "--no-color",
                "diff",
                "--left-query",
                "SELECT order_id, amount_cents FROM prod.orders_snapshot",
                "--right-query",
                "SELECT order_id, amount_cents FROM dev.orders_snapshot",
                "--key",
                "order_id",
                "--tolerance",
                "amount_cents:absolute=5",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=("amount_cents absolute=5", "No changed columns."),
        ),
        DiffCommandE2ETestCase(
            description="query exclusions suppress ignored column differences",
            mutation_sql=("UPDATE dev.orders_snapshot SET status = 'fulfilled'",),
            command=(
                "--no-color",
                "diff",
                "--left-query",
                "SELECT order_id, status FROM prod.orders_snapshot",
                "--right-query",
                "SELECT order_id, status FROM dev.orders_snapshot",
                "--key",
                "order_id",
                "--exclude-column",
                "status",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=("Excluded", "status", "No changed columns."),
        ),
        DiffCommandE2ETestCase(
            description="query composite keys preserve row identity",
            command=(
                "--no-color",
                "diff",
                "--left-query",
                "SELECT customer_id, order_id, amount_cents FROM prod.orders_snapshot",
                "--right-query",
                "SELECT customer_id, order_id, amount_cents FROM dev.orders_snapshot",
                "--key",
                "customer_id",
                "--key",
                "order_id",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=("customer_id, order_id", "No changed columns."),
        ),
        DiffCommandE2ETestCase(
            description="read-only set queries are accepted",
            command=(
                "--no-color",
                "diff",
                "--left-query",
                "SELECT 1 AS order_id UNION ALL SELECT 2 AS order_id",
                "--right-query",
                "SELECT 1 AS order_id UNION ALL SELECT 2 AS order_id",
                "--key",
                "order_id",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=("No changed columns.",),
        ),
        DiffCommandE2ETestCase(
            description="query key integrity rejects duplicate identities",
            command=(
                "--no-color",
                "diff",
                "--left-query",
                "SELECT * FROM (VALUES (1, 100), (1, 200)) AS orders(order_id, amount_cents)",
                "--right-query",
                "SELECT * FROM (VALUES (1, 100)) AS orders(order_id, amount_cents)",
                "--key",
                "order_id",
            ),
            expected_exit_code=2,
            expected_stderr_fragments=("contains duplicate unique_key values",),
        ),
        DiffCommandE2ETestCase(
            description="query labels and diff aware truncation keep long evidence compact",
            command=(
                "--no-color",
                "diff",
                "--left-query",
                "SELECT 1 AS order_id, '" + ("a" * 200) + "left-tail' AS note",
                "--right-query",
                "SELECT 1 AS order_id, '" + ("a" * 200) + "right-tail' AS note",
                "--left-label",
                "baseline",
                "--right-label",
                "candidate",
                "--key",
                "order_id",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "baseline vs candidate",
                "baseline count",
                "candidate count",
                "[truncated; 209 chars]",
                "first difference at character 201",
            ),
        ),
        DiffCommandE2ETestCase(
            description="query examples can suppress values without suppressing keys",
            command=(
                "--no-color",
                "diff",
                "--left-query",
                "SELECT 1 AS order_id, 'left-private-value' AS note",
                "--right-query",
                "SELECT 1 AS order_id, 'right-private-value' AS note",
                "--key",
                "order_id",
                "--no-example-values",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=("order_id=1", "<value suppressed; 18 chars>"),
            unexpected_stdout_fragments=("left-private-value", "right-private-value"),
        ),
        DiffCommandE2ETestCase(
            description="complete example values require explicit opt in",
            command=(
                "--no-color",
                "diff",
                "--left-query",
                "SELECT 1 AS order_id, '" + ("a" * 200) + "left-tail' AS note",
                "--right-query",
                "SELECT 1 AS order_id, '" + ("a" * 200) + "right-tail' AS note",
                "--key",
                "order_id",
                "--full-example-values",
            ),
            expected_exit_code=1,
            expected_stdout_fragments=("left-tail", "right-tail"),
            unexpected_stdout_fragments=("[truncated;",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_raw_queries_when_running_diff_then_comparison_matches_model_diff_capabilities(
    test_case: DiffCommandE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_diff_project(tmp_path)
    build_both_environments(project_dir=project_dir)
    for mutation_sql in test_case.mutation_sql:
        execute_duckdb(db_path=project_dir / "diff.duckdb", sql=mutation_sql)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout, result.stdout + result.stderr
    for fragment in test_case.expected_stderr_fragments:
        assert fragment in result.stderr, result.stdout + result.stderr
    for fragment in test_case.unexpected_stdout_fragments:
        assert fragment not in result.stdout, result.stdout + result.stderr
    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(project_dir / "diff.duckdb"))
    try:
        artifact_row: tuple[int] | None = connection.execute(
            "SELECT COUNT(*) FROM duckdb_tables() WHERE table_name LIKE '__sqlbuild_query_diff_%'"
        ).fetchone()
        fingerprint_row: tuple[int] | None = connection.execute(
            "SELECT COUNT(*) FROM dev._sqlbuild_fingerprints "
            "WHERE node_type = 'query_diff_artifact'"
        ).fetchone()
    finally:
        connection.close()
    assert artifact_row == (0,)
    assert fingerprint_row == (2,)


@pytest.mark.parametrize(
    "test_case",
    [
        SingleDiffE2ETestCase(
            description="query files target and JSON output",
            expected_exit_code=0,
            expected_fragment="no_differences_found",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_query_files_target_and_json_output_when_running_diff_then_all_inputs_are_honored(
    test_case: SingleDiffE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_diff_project(tmp_path)
    build_both_environments(project_dir=project_dir)
    (project_dir / "left.sql").write_text(
        "SELECT order_id, amount_cents FROM prod.orders_snapshot;\n",
        encoding="utf-8",
    )
    (project_dir / "right.sql").write_text(
        "SELECT order_id, amount_cents FROM dev.orders_snapshot;\n",
        encoding="utf-8",
    )
    json_path: Path = project_dir / "query-diff.json"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "diff",
            "--left-query-file",
            str(project_dir / "left.sql"),
            "--right-query-file",
            str(project_dir / "right.sql"),
            "--target",
            "prod",
            "--key",
            "order_id",
            "--json-output",
            str(json_path),
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    payload: dict[str, object] = cast(dict[str, object], json.loads(json_path.read_text()))
    assert payload["status"] == test_case.expected_fragment
    models: list[dict[str, object]] = cast(list[dict[str, object]], payload["models"])
    assert models[0]["input_kind"] == "query"
    connection: duckdb.DuckDBPyConnection = duckdb.connect(str(project_dir / "diff.duckdb"))
    try:
        prod_fingerprint_row: tuple[int] | None = connection.execute(
            "SELECT COUNT(*) FROM prod._sqlbuild_fingerprints "
            "WHERE node_type = 'query_diff_artifact'"
        ).fetchone()
        dev_fingerprint_row: tuple[int] | None = connection.execute(
            "SELECT COUNT(*) FROM dev._sqlbuild_fingerprints "
            "WHERE node_type = 'query_diff_artifact'"
        ).fetchone()
    finally:
        connection.close()
    assert prod_fingerprint_row == (2,)
    assert dev_fingerprint_row == (0,)


@pytest.mark.parametrize(
    "test_case",
    [
        SingleDiffE2ETestCase(
            description="mutating CTE rejection",
            expected_exit_code=2,
            expected_fragment="must not contain data-changing or administrative statements",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_mutating_cte_when_running_query_diff_then_validation_rejects_it(
    test_case: SingleDiffE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_diff_project(tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "diff",
            "--left-query",
            "WITH changed AS (DELETE FROM orders RETURNING *) SELECT * FROM changed",
            "--right-query",
            "SELECT 1 AS order_id",
            "--key",
            "order_id",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code
    assert test_case.expected_fragment in result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        SingleDiffE2ETestCase(
            description="duplicate query column rejection",
            expected_exit_code=2,
            expected_fragment="left query returns duplicate column names: order_id",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_duplicate_query_columns_when_running_query_diff_then_preflight_rejects_them(
    test_case: SingleDiffE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_diff_project(tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "diff",
            "--left-query",
            "SELECT 1 AS order_id, 100 AS order_id",
            "--right-query",
            "SELECT 1 AS order_id, 100 AS amount_cents",
            "--key",
            "order_id",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code
    assert test_case.expected_fragment in result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        SingleDiffE2ETestCase(
            description="historical fingerprint isolation",
            expected_exit_code=0,
            expected_fragment="recovery cleanup could not inspect historical ownership evidence",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unreadable_historical_fingerprint_when_running_query_diff_then_comparison_continues(
    test_case: SingleDiffE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_diff_project(tmp_path)
    build_both_environments(project_dir=project_dir)
    execute_duckdb(
        db_path=project_dir / "diff.duckdb",
        sql=(
            "CREATE TABLE dev.__sqlbuild_query_diff_20260917T120000Z_a1b2c3d4e5f6_left "
            "AS SELECT 1 AS order_id"
        ),
    )
    execute_duckdb(
        db_path=project_dir / "diff.duckdb",
        sql=(
            "UPDATE dev._sqlbuild_fingerprints SET definition_b64 = 'not-base64' "
            "WHERE node_type = 'model'"
        ),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "diff",
            "--left-query",
            "SELECT order_id FROM prod.orders_snapshot",
            "--right-query",
            "SELECT order_id FROM dev.orders_snapshot",
            "--key",
            "order_id",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert test_case.expected_fragment in result.stderr
    assert "No changed columns." in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        SingleDiffE2ETestCase(
            description="query JSON stdout", expected_exit_code=1, expected_fragment="findings"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_json_stdout_when_running_labeled_query_diff_then_stdout_is_one_query_native_document(
    test_case: SingleDiffE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_diff_project(tmp_path)
    build_both_environments(project_dir=project_dir)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "diff",
            "--left-query",
            "SELECT 'NaN'::DOUBLE AS order_id, '" + ("a" * 30) + "left-tail' AS note",
            "--right-query",
            "SELECT 'NaN'::DOUBLE AS order_id, '" + ("a" * 30) + "right-tail' AS note",
            "--left-label",
            "baseline",
            "--right-label",
            "candidate",
            "--key",
            "order_id",
            "--max-value-length",
            "12",
            "--json",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    payload: dict[str, object] = cast(
        dict[str, object],
        json.loads(result.stdout, parse_constant=reject_nonfinite_json_constant),
    )
    assert payload["outcome"] == test_case.expected_fragment
    assert payload["from"] == "baseline"
    assert payload["to"] == "candidate"
    assert payload["input_kind"] == "query"
    comparisons: list[dict[str, object]] = cast(
        list[dict[str, object]], payload["query_comparisons"]
    )
    rows: dict[str, object] = cast(dict[str, object], comparisons[0]["rows"])
    assert rows["column_mismatches"] == [{"column": "note", "rows": 1}]
    examples: dict[str, object] = cast(dict[str, object], comparisons[0]["examples"])
    assert examples["values_truncated"] is True
    assert examples["max_value_length"] == 12
    unequal_rows: list[dict[str, object]] = cast(list[dict[str, object]], examples["unequal_rows"])
    example_key: dict[str, object] = cast(dict[str, object], unequal_rows[0]["key"])
    assert example_key["order_id"] == {"kind": "non_finite_number", "value": "NaN"}
    assert "phase_seconds" in payload
    assert "Query diff total  OK" in result.stderr
    assert "SQLBuild Diff Summary" not in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        SingleDiffE2ETestCase(
            description="query JSON output publication failure",
            expected_exit_code=3,
            expected_fragment="failed to publish query diff output",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unwritable_json_path_when_running_query_diff_then_stdout_reports_execution_failure(
    test_case: SingleDiffE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_diff_project(tmp_path)
    blocked_parent: Path = project_dir / "blocked-output"
    blocked_parent.write_text("not a directory\n", encoding="utf-8")

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "diff",
            "--left-query",
            "SELECT 1 AS order_id",
            "--right-query",
            "SELECT 1 AS order_id",
            "--key",
            "order_id",
            "--json",
            "--json-output",
            str(blocked_parent / "result.json"),
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    payload: dict[str, object] = cast(
        dict[str, object],
        json.loads(result.stdout, parse_constant=reject_nonfinite_json_constant),
    )
    assert payload["outcome"] == "execution_failed"
    error: dict[str, object] = cast(dict[str, object], payload["error"])
    assert test_case.expected_fragment in str(error["message"])
    assert "Query diff outcome  execution_failed" in result.stderr
    assert "Traceback" not in result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        SingleDiffE2ETestCase(
            description="lightweight raw query discovery",
            expected_exit_code=0,
            expected_fragment="No changed columns.",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unimportable_project_extension_when_running_raw_query_diff_then_it_is_not_loaded(
    test_case: SingleDiffE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_diff_project(tmp_path)
    build_both_environments(project_dir=project_dir)
    sink_path: Path = project_dir / "python" / "event_sinks" / "broken_sink.py"
    sink_path.parent.mkdir(parents=True, exist_ok=True)
    sink_path.write_text("import package_that_is_not_installed\n", encoding="utf-8")

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "diff",
            "--left-query",
            "SELECT 1 AS order_id",
            "--right-query",
            "SELECT 1 AS order_id",
            "--key",
            "order_id",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert test_case.expected_fragment in result.stdout
    assert "package_that_is_not_installed" not in result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        QueryDiffOutcomeE2ETestCase(
            description="duplicate keys are incomplete evidence",
            left_query="SELECT * FROM (VALUES (1), (1)) AS orders(order_id)",
            right_query="SELECT * FROM (VALUES (1)) AS orders(order_id)",
            expected_exit_code=2,
            expected_status="incomplete",
            expected_error_fragment="duplicate unique_key values",
        ),
        QueryDiffOutcomeE2ETestCase(
            description="warehouse query failures are execution failures",
            left_query="SELECT order_id FROM missing_orders",
            right_query="SELECT 1 AS order_id",
            expected_exit_code=3,
            expected_status="execution_failed",
            expected_error_fragment="missing_orders",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_query_diff_cannot_complete_when_requesting_json_then_outcome_is_machine_readable(
    test_case: QueryDiffOutcomeE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_diff_project(tmp_path)
    build_both_environments(project_dir=project_dir)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "diff",
            "--left-query",
            test_case.left_query,
            "--right-query",
            test_case.right_query,
            "--key",
            "order_id",
            "--json",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    payload: dict[str, object] = cast(dict[str, object], json.loads(result.stdout))
    assert payload["status"] == test_case.expected_status
    assert payload["outcome"] == test_case.expected_status
    error: dict[str, object] = cast(dict[str, object], payload["error"])
    assert test_case.expected_error_fragment.lower() in str(error["message"]).lower()
    assert f"Query diff outcome  {test_case.expected_status}" in result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        DiffSamplingPrecedenceE2ETestCase(
            description="model and cli sampling override path and project defaults",
            project_limit=1,
            project_seed=3,
            path_limit=2,
            path_seed=5,
            model_limit=3,
            model_seed=7,
            cli_limit=2,
            cli_seed=11,
            expected_model_scope="exhaustive (3 union keys; configured sample limit 3)",
            expected_model_exhaustive_scope="exhaustive (3 union keys)",
            expected_path_scope="sampled 2 of 3 union keys (66.67%; seed 5)",
            expected_project_scope="sampled 1 of 3 union keys (33.33%; seed 3)",
            expected_cli_scope="sampled 2 of 3 union keys (66.67%; seed 11)",
            expected_exhaustive_scope="exhaustive (3 union keys)",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_layered_sampling_config_when_diffing_then_model_and_cli_precedence_apply(
    test_case: DiffSamplingPrecedenceE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_diff_project(tmp_path)
    project_config_path: Path = project_dir / "sqlbuild_project.toml"
    project_config: str = project_config_path.read_text(encoding="utf-8")
    project_config = project_config.replace(
        '[defaults]\nmaterialized = "table"',
        f'[defaults]\nmaterialized = "table"\nrow_diff_sample_rows = '
        f"{test_case.project_limit}\nrow_diff_sample_seed = {test_case.project_seed}",
    )
    project_config += (
        "\n[path_defaults.intermediate]\n"
        f"row_diff_sample_rows = {test_case.path_limit}\n"
        f"row_diff_sample_seed = {test_case.path_seed}\n"
    )
    project_config_path.write_text(project_config, encoding="utf-8")
    model_path: Path = project_dir / "sql/models/intermediate/orders_snapshot.sql"
    model_sql: str = model_path.read_text(encoding="utf-8")
    model_sql = model_sql.replace(
        "  cursor_type timestamp,",
        f"  cursor_type timestamp,\n  row_diff_sample_rows {test_case.model_limit},\n"
        f"  row_diff_sample_seed {test_case.model_seed},",
    )
    model_path.write_text(model_sql, encoding="utf-8")
    build_both_environments(project_dir=project_dir)

    model_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "diff", "prod:dev", "--full", "--select", "orders_snapshot"),
        project_dir=project_dir,
    )

    assert model_result.returncode == 0, model_result.stdout + model_result.stderr
    assert test_case.expected_model_scope in model_result.stdout

    inherited_disabled_sql: str = model_sql.replace(
        f"  row_diff_sample_rows {test_case.model_limit},\n"
        f"  row_diff_sample_seed {test_case.model_seed},",
        "  row_diff_sample_rows 0,",
    )
    model_path.write_text(inherited_disabled_sql, encoding="utf-8")
    model_exhaustive_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "diff", "prod:dev", "--full", "--select", "orders_snapshot"),
        project_dir=project_dir,
    )

    assert model_exhaustive_result.returncode == 0, (
        model_exhaustive_result.stdout + model_exhaustive_result.stderr
    )
    assert test_case.expected_model_exhaustive_scope in model_exhaustive_result.stdout
    assert "configured sample limit" not in model_exhaustive_result.stdout

    model_path.write_text(
        model_sql.replace(
            f"  row_diff_sample_rows {test_case.model_limit},\n"
            f"  row_diff_sample_seed {test_case.model_seed},\n",
            "",
        ),
        encoding="utf-8",
    )
    path_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "diff", "prod:dev", "--full", "--select", "orders_snapshot"),
        project_dir=project_dir,
    )

    assert path_result.returncode == 0, path_result.stdout + path_result.stderr
    assert test_case.expected_path_scope in path_result.stdout

    project_config_path.write_text(
        project_config.replace(
            "\n[path_defaults.intermediate]\n"
            f"row_diff_sample_rows = {test_case.path_limit}\n"
            f"row_diff_sample_seed = {test_case.path_seed}\n",
            "\n",
        ),
        encoding="utf-8",
    )
    project_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "diff", "prod:dev", "--full", "--select", "orders_snapshot"),
        project_dir=project_dir,
    )

    assert project_result.returncode == 0, project_result.stdout + project_result.stderr
    assert test_case.expected_project_scope in project_result.stdout

    cli_result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "diff",
            "prod:dev",
            "--full",
            "--sample-rows",
            str(test_case.cli_limit),
            "--sample-seed",
            str(test_case.cli_seed),
            "--select",
            "orders_snapshot",
        ),
        project_dir=project_dir,
    )

    assert cli_result.returncode == 0, cli_result.stdout + cli_result.stderr
    assert test_case.expected_cli_scope in cli_result.stdout

    exhaustive_result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "diff",
            "prod:dev",
            "--full",
            "--exhaustive",
            "--select",
            "orders_snapshot",
        ),
        project_dir=project_dir,
    )

    assert exhaustive_result.returncode == 0, exhaustive_result.stdout + exhaustive_result.stderr
    assert test_case.expected_exhaustive_scope in exhaustive_result.stdout
    assert "configured sample limit" not in exhaustive_result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        DiffSamplingSeedE2ETestCase(
            description="same seed repeats membership and alternate seed changes it",
            row_limit=2,
            repeated_seed=7,
            alternate_seed=11,
            expected_membership_count=2,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_changed_population_when_sampling_then_seed_membership_is_reproducible(
    test_case: DiffSamplingSeedE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_diff_project(tmp_path)
    build_both_environments(project_dir=project_dir)
    execute_duckdb(
        db_path=project_dir / "diff.duckdb",
        sql="UPDATE dev.orders_snapshot SET amount_cents = amount_cents + 10",
    )

    first_result: subprocess.CompletedProcess[str] = run_sampled_diff(
        project_dir=project_dir,
        model_name="orders_snapshot",
        row_limit=test_case.row_limit,
        seed=test_case.repeated_seed,
    )
    repeated_result: subprocess.CompletedProcess[str] = run_sampled_diff(
        project_dir=project_dir,
        model_name="orders_snapshot",
        row_limit=test_case.row_limit,
        seed=test_case.repeated_seed,
    )
    alternate_result: subprocess.CompletedProcess[str] = run_sampled_diff(
        project_dir=project_dir,
        model_name="orders_snapshot",
        row_limit=test_case.row_limit,
        seed=test_case.alternate_seed,
    )

    assert first_result.returncode == 1, first_result.stdout + first_result.stderr
    assert repeated_result.returncode == 1, repeated_result.stdout + repeated_result.stderr
    assert alternate_result.returncode == 1, alternate_result.stdout + alternate_result.stderr
    first_keys: tuple[str, ...] = changed_key_examples(output=first_result.stdout)
    repeated_keys: tuple[str, ...] = changed_key_examples(output=repeated_result.stdout)
    alternate_keys: tuple[str, ...] = changed_key_examples(output=alternate_result.stdout)
    assert first_keys == repeated_keys
    assert first_keys != alternate_keys
    assert len(first_keys) == test_case.expected_membership_count
    assert len(alternate_keys) == test_case.expected_membership_count


@pytest.mark.parametrize(
    "test_case",
    [
        DiffJsonOutputE2ETestCase(
            description="sampled diff writes explicit structured scope and coverage",
            row_limit=2,
            seed=7,
            expected_status="no_differences_found",
            expected_scope="sampled",
            expected_population=3,
            expected_compared=2,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_json_output_path_when_sampling_then_document_reports_evaluated_scope(
    test_case: DiffJsonOutputE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_diff_project(tmp_path)
    build_both_environments(project_dir=project_dir)
    output_path: Path = project_dir / "diff-result.json"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "diff",
            "prod:dev",
            "--full",
            "--sample-rows",
            str(test_case.row_limit),
            "--sample-seed",
            str(test_case.seed),
            "--json-output",
            str(output_path),
            "--select",
            "orders_snapshot",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    payload: dict[str, object] = cast(
        dict[str, object], json.loads(output_path.read_text(encoding="utf-8"))
    )
    assert payload["status"] == test_case.expected_status
    models: list[dict[str, object]] = cast(list[dict[str, object]], payload["models"])
    model: dict[str, object] = models[0]
    assert model["comparison_scope"] == test_case.expected_scope
    sampling: dict[str, object] = cast(dict[str, object], model["sampling"])
    assert sampling["population_keys"] == test_case.expected_population
    assert sampling["compared_keys"] == test_case.expected_compared
    coverage: dict[str, object] = cast(dict[str, object], model["coverage"])
    from_coverage: dict[str, object] = cast(dict[str, object], coverage["from"])
    to_coverage: dict[str, object] = cast(dict[str, object], coverage["to"])
    assert from_coverage["row_count"] == test_case.expected_population
    assert to_coverage["row_count"] == test_case.expected_population


@pytest.mark.parametrize(
    "test_case",
    [
        DiffCommandE2ETestCase(
            description="schema diff ignores unused from connection credentials",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--schema-only",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=("No schema differences.",),
        ),
        DiffCommandE2ETestCase(
            description="full diff ignores unused from connection credentials",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--full",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=("No changed columns.",),
        ),
        DiffCommandE2ETestCase(
            description="bounded diff ignores unused from connection credentials",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--bounded",
                "30d",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=0,
            expected_stdout_fragments=("No changed columns.",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unused_from_connection_when_diffing_then_to_connection_is_authoritative(
    test_case: DiffCommandE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    project_dir: Path = prepare_diff_project(tmp_path)
    build_both_environments(project_dir=project_dir)
    project_config_path: Path = project_dir / "sqlbuild_project.toml"
    original_project_config: str = project_config_path.read_text(encoding="utf-8")
    rewritten_project_config: str = original_project_config.replace(
        '[targets.prod]\nschema = "prod"',
        '[targets.prod]\nschema = "prod"\n\n[targets.prod.connection]\n'
        'database = "${ENV:SQLBUILD_TEST_UNUSED_FROM_DATABASE}"',
    )
    assert rewritten_project_config != original_project_config
    project_config_path.write_text(
        rewritten_project_config,
        encoding="utf-8",
    )
    monkeypatch.delenv("SQLBUILD_TEST_UNUSED_FROM_DATABASE", raising=False)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout, result.stdout + result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        DiffCommandE2ETestCase(
            description="missing to connection credential fails before inspection",
            command=(
                "--no-color",
                "diff",
                "prod:dev",
                "--schema-only",
                "--select",
                "orders_snapshot",
            ),
            expected_exit_code=1,
            expected_stderr_fragments=(
                "effective connection references missing ENV variable "
                "'SQLBUILD_TEST_MISSING_TO_DATABASE'",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_missing_to_connection_credential_when_diffing_then_it_fails_before_inspection(
    test_case: DiffCommandE2ETestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    project_dir: Path = prepare_diff_project(tmp_path)
    build_both_environments(project_dir=project_dir)
    project_config_path: Path = project_dir / "sqlbuild_project.toml"
    original_project_config: str = project_config_path.read_text(encoding="utf-8")
    rewritten_project_config: str = original_project_config.replace(
        '[targets.dev]\nschema = "dev"',
        '[targets.dev]\nschema = "dev"\n\n[targets.dev.connection]\n'
        'database = "${ENV:SQLBUILD_TEST_MISSING_TO_DATABASE}"',
    )
    assert rewritten_project_config != original_project_config
    project_config_path.write_text(rewritten_project_config, encoding="utf-8")
    monkeypatch.delenv("SQLBUILD_TEST_MISSING_TO_DATABASE", raising=False)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    for fragment in test_case.expected_stderr_fragments:
        assert fragment in result.stderr, result.stdout + result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        DiffKeyFailureE2ETestCase(
            description="null unique key fails clearly",
            mutation_sql=("UPDATE dev.orders_snapshot SET order_id = NULL WHERE order_id = 3",),
            expected_stderr_fragment="row diff right relation contains null unique_key values",
        ),
        DiffKeyFailureE2ETestCase(
            description="duplicate unique key fails clearly",
            mutation_sql=("UPDATE dev.orders_snapshot SET order_id = 2 WHERE order_id = 3",),
            expected_stderr_fragment="row diff right relation contains duplicate unique_key values",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_dev_keys_when_running_diff_then_it_fails_clearly(
    test_case: DiffKeyFailureE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_diff_project(tmp_path)
    build_both_environments(project_dir=project_dir)
    db_path: Path = project_dir / "diff.duckdb"

    mutation_sql: str
    for mutation_sql in test_case.mutation_sql:
        execute_duckdb(db_path=db_path, sql=mutation_sql)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "diff",
            "prod:dev",
            "--full",
            "--select",
            "orders_snapshot",
        ),
        project_dir=project_dir,
    )

    assert result.returncode == 1, result.stdout + result.stderr
    assert test_case.expected_stderr_fragment in result.stderr, result.stdout + result.stderr
