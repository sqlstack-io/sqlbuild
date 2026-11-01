"""Real CLI and DuckDB coverage for the borrowed query graph."""

from __future__ import annotations

import json
from pathlib import Path
from typing import cast

import duckdb
import pytest

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    AliasSourceCompileTestCase,
    BoundProjectionCompileTestCase,
    DerivedNativeCompileTestCase,
    KeywordFunctionCompileTestCase,
    SnapshotValidityCompileTestCase,
    StarExpansionCompileTestCase,
    StarLineageCompileTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import compile_duckdb_query

# Cases that check facts through derived tables turn off the Rule that asks for a CTE instead.
_DERIVED_RELATION_PROJECT: str = (
    'name = "orders"\nadapter = "duckdb"\n\n[rules]\nignore = ["SQLCLEAN039"]\n'
)


@pytest.mark.parametrize(
    "test_case",
    (
        DerivedNativeCompileTestCase(
            description="derived-table column aliases preserve the declared contract",
            query_sql=(
                "WITH typed AS (SELECT CAST(1 AS BIGINT) AS original_id) "
                "SELECT order_id FROM (SELECT original_id FROM typed) AS nested_orders(order_id)"
            ),
            expected_exit_code=0,
            expected_diagnostics=(),
            expected_rows=((1,),),
        ),
        DerivedNativeCompileTestCase(
            description="CTE column aliases survive a derived-table star",
            query_sql=(
                "WITH typed(order_id) AS (SELECT CAST(1 AS BIGINT) AS original_id) "
                "SELECT * FROM (SELECT order_id FROM typed) AS nested_orders"
            ),
            expected_exit_code=0,
            expected_diagnostics=(),
            expected_rows=((1,),),
        ),
        DerivedNativeCompileTestCase(
            description="both named-union branches survive a derived table",
            query_sql=(
                "WITH combined AS (SELECT CAST(1 AS BIGINT) AS order_id "
                "UNION ALL BY NAME SELECT CAST(2 AS BIGINT) AS order_id) "
                "SELECT * FROM (SELECT order_id FROM combined) AS nested_orders ORDER BY order_id"
            ),
            expected_exit_code=0,
            expected_diagnostics=(),
            expected_rows=((1,), (2,)),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_derived_native_facts_when_compiling_then_contract_and_execution_agree(
    test_case: DerivedNativeCompileTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(_DERIVED_RELATION_PROJECT)
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "orders.sql").write_text(
        "MODEL (description 'Test model orders.', contract enforced, columns (order_id (type BIGINT, nullable false)));\n"
        + test_case.query_sql
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])
    result: dict[str, object] = json.loads(capsys.readouterr().out)
    diagnostics: list[dict[str, object]] = cast(list[dict[str, object]], result["diagnostics"])

    assert exit_code == test_case.expected_exit_code
    assert tuple(item["code"] for item in diagnostics) == test_case.expected_diagnostics
    compiled_sql: str = (tmp_path / "target" / "compiled" / "models" / "orders.sql").read_text()
    with duckdb.connect() as connection:
        rows: tuple[tuple[int, ...], ...] = tuple(connection.execute(compiled_sql).fetchall())
    assert rows == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    (
        StarExpansionCompileTestCase(
            description="qualified star beside a joined column expands for downstream readers",
            star_model_sql=(
                'SELECT staged.*, extra.status FROM __ref("staged_orders") AS staged '
                "CROSS JOIN (SELECT 'open' AS status) AS extra"
            ),
            downstream_sql='SELECT order_id, status FROM __ref("wide_orders")',
            expected_exit_code=0,
            expected_diagnostics=(),
        ),
        StarExpansionCompileTestCase(
            description="bare star over an inferred upstream still rejects unknown columns",
            star_model_sql='SELECT * FROM __ref("staged_orders")',
            downstream_sql='SELECT order_id, missing_column FROM __ref("wide_orders")',
            expected_exit_code=1,
            expected_diagnostics=("B002",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_star_over_inferred_upstream_when_compiling_then_downstream_binding_is_exact(
    test_case: StarExpansionCompileTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(_DERIVED_RELATION_PROJECT)
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "staged_orders.sql").write_text(
        "MODEL (description 'Test model staged_orders.');\nSELECT 1 AS order_id"
    )
    (models / "wide_orders.sql").write_text(
        f"MODEL (description 'Test model wide_orders.');\n{test_case.star_model_sql}"
    )
    (models / "order_readers.sql").write_text(
        f"MODEL (description 'Test model order_readers.');\n{test_case.downstream_sql}"
    )

    for _ in ("cold", "warm"):
        exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json"])
        result: dict[str, object] = json.loads(capsys.readouterr().out)
        diagnostics: list[dict[str, object]] = cast(list[dict[str, object]], result["diagnostics"])

        assert exit_code == test_case.expected_exit_code
        assert tuple(item["code"] for item in diagnostics) == test_case.expected_diagnostics


@pytest.mark.parametrize(
    "test_case",
    (
        StarLineageCompileTestCase(
            description="star over a joined derived table omits unprojected seed columns",
            star_model_sql=(
                "SELECT * FROM (SELECT input.order_id, amount "
                'FROM __ref("staged_orders") AS input '
                'LEFT JOIN __seed("order_labels") AS labels '
                "ON labels.order_id = input.order_id) AS nested_orders"
            ),
            expected_edge_count=2,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_star_over_derived_table_when_compiling_then_lineage_matches_projection(
    test_case: StarLineageCompileTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(_DERIVED_RELATION_PROJECT)
    seeds: Path = tmp_path / "seeds"
    seeds.mkdir()
    (seeds / "order_labels.csv").write_text("order_id,label\n1,first\n")
    (seeds / "order_labels.yml").write_text(
        "seeds:\n  - name: order_labels\n    description: Test seed order_labels.\n    columns:\n"
        "      - name: order_id\n        type: INTEGER\n"
        "      - name: label\n        type: VARCHAR\n"
    )
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "staged_orders.sql").write_text(
        "MODEL (description 'Test model staged_orders.');\nSELECT 1 AS order_id, CAST(10 AS DOUBLE) AS amount"
    )
    (models / "wide_orders.sql").write_text(
        f"MODEL (description 'Test model wide_orders.');\n{test_case.star_model_sql}"
    )

    for _ in ("cold", "warm"):
        exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json"])
        result: dict[str, object] = json.loads(capsys.readouterr().out)
        resources: dict[str, object] = cast(dict[str, object], result["resources"])
        compiled_models: list[dict[str, object]] = cast(
            list[dict[str, object]], resources["models"]
        )
        lineage_by_model: dict[object, dict[str, object]] = {
            model["name"]: cast(dict[str, object], model["lineage"]) for model in compiled_models
        }
        lineage: dict[str, object] = lineage_by_model["wide_orders"]

        assert exit_code == 0
        assert result["diagnostics"] == []
        # order_id and amount each trace to staged_orders; the joined seed's label is
        # not projected by the derived table, so it contributes no edge.
        assert lineage["edge_count"] == test_case.expected_edge_count


@pytest.mark.parametrize(
    "test_case",
    (
        SnapshotValidityCompileTestCase(
            description="default validity columns are readable downstream",
            snapshot_config="",
            downstream_sql=(
                'SELECT customer_id, valid_from FROM __ref("customer_snapshot") '
                "WHERE valid_to IS NULL"
            ),
            expected_exit_code=0,
            expected_diagnostics=(),
        ),
        SnapshotValidityCompileTestCase(
            description="renamed validity columns are readable downstream",
            snapshot_config=", valid_from_column active_from, valid_to_column active_to",
            downstream_sql=(
                'SELECT customer_id, active_from FROM __ref("customer_snapshot") '
                "WHERE active_to IS NULL"
            ),
            expected_exit_code=0,
            expected_diagnostics=(),
        ),
        SnapshotValidityCompileTestCase(
            description="unknown snapshot columns are still rejected",
            snapshot_config="",
            downstream_sql='SELECT customer_id, missing_column FROM __ref("customer_snapshot")',
            expected_exit_code=1,
            expected_diagnostics=("B002",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_snapshot_reader_when_compiling_then_validity_columns_are_known(
    test_case: SnapshotValidityCompileTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "duckdb"\n')
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "customer_snapshot.sql").write_text(
        "MODEL (description 'Test model customer_snapshot.', materialized snapshot, unique_key [customer_id], snapshot_strategy timestamp, "
        f"updated_at updated_at{test_case.snapshot_config});\n"
        "SELECT 1 AS customer_id, 'pro' AS plan, TIMESTAMP '2026-01-01' AS updated_at"
    )
    (models / "current_customers.sql").write_text(
        f"MODEL (description 'Test model current_customers.');\n{test_case.downstream_sql}"
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])
    result: dict[str, object] = json.loads(capsys.readouterr().out)
    diagnostics: list[dict[str, object]] = cast(list[dict[str, object]], result["diagnostics"])

    assert exit_code == test_case.expected_exit_code
    assert tuple(item["code"] for item in diagnostics) == test_case.expected_diagnostics


@pytest.mark.parametrize(
    "test_case",
    (
        KeywordFunctionCompileTestCase(
            description="keyword timestamps compile and execute",
            query=(
                "SELECT CURRENT_TIMESTAMP AS created_at, LOCALTIMESTAMP AS local_created_at, "
                "CURRENT_TIME AS created_time, LOCALTIME AS local_created_time"
            ),
            expected_exit_code=0,
            expected_diagnostics=(),
        ),
        KeywordFunctionCompileTestCase(
            description="keyword timestamp in a CTE arithmetic filter",
            query=(
                "WITH recent AS (SELECT CURRENT_TIMESTAMP AS created_at) "
                "SELECT created_at FROM recent WHERE created_at > CURRENT_TIMESTAMP - INTERVAL 1 DAY"
            ),
            expected_exit_code=0,
            expected_diagnostics=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_duckdb_keyword_function_when_compiling_then_compiled_sql_executes(
    test_case: KeywordFunctionCompileTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    exit_code, codes = compile_duckdb_query(tmp_path=tmp_path, capsys=capsys, query=test_case.query)

    assert exit_code == test_case.expected_exit_code
    assert codes == test_case.expected_diagnostics
    compiled_sql: str = (tmp_path / "target" / "compiled" / "models" / "orders.sql").read_text()
    with duckdb.connect() as connection:
        assert len(connection.execute(compiled_sql).fetchall()) == 1


@pytest.mark.parametrize(
    "test_case",
    (
        KeywordFunctionCompileTestCase(
            description="parenthesized keyword timestamp is still unknown",
            query="SELECT CURRENT_TIMESTAMP() AS created_at",
            expected_exit_code=1,
            expected_diagnostics=("B101",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_duckdb_keyword_call_when_compiling_then_function_is_rejected(
    test_case: KeywordFunctionCompileTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    exit_code, codes = compile_duckdb_query(tmp_path=tmp_path, capsys=capsys, query=test_case.query)

    assert exit_code == test_case.expected_exit_code
    assert codes == test_case.expected_diagnostics


@pytest.mark.parametrize(
    "test_case",
    (
        AliasSourceCompileTestCase(
            description="direct source",
            query_prefix="",
            source_relation='__source("orders")',
            expected_edge_count=5,
        ),
        AliasSourceCompileTestCase(
            description="CTE source",
            query_prefix='WITH source_rows AS (SELECT * FROM __source("orders")) ',
            source_relation="source_rows",
            expected_edge_count=5,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_input_and_output_share_name_when_compiling_then_lineage_keeps_input_dependency(
    test_case: AliasSourceCompileTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text('name = "orders"\nadapter = "duckdb"\n')
    models: Path = tmp_path / "models"
    models.mkdir()
    sources: Path = tmp_path / "sources"
    sources.mkdir()
    (sources / "orders.yml").write_text(
        "sources:\n  - name: orders\n    description: Test source orders.\n    contract: enforced\n"
        "    expression: orders_input\n    columns:\n"
        "      - name: order_id\n        type: INTEGER\n"
        "      - name: amount\n        type: DOUBLE\n"
        "      - name: status\n        type: VARCHAR\n"
    )
    (models / "order_totals.sql").write_text(
        "MODEL (description 'Test model order_totals.', contract enforced, columns (order_id (type INTEGER), "
        "status (type VARCHAR), total (type DOUBLE)));\n"
        f"{test_case.query_prefix}"
        "SELECT order_id, "
        "CAST(CASE WHEN order_id % 2 = 0 THEN 'even' ELSE 'odd' END AS VARCHAR) AS status, "
        "CAST(CASE WHEN order_id % 2 = 0 THEN amount + 2 "
        "WHEN status = 'priority' THEN amount * 2 ELSE amount - 1 END AS DOUBLE) AS total "
        f"FROM {test_case.source_relation} AS input"
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])
    result: dict[str, object] = json.loads(capsys.readouterr().out)
    assert exit_code == 0
    assert result["diagnostics"] == []
    resources: dict[str, object] = cast(dict[str, object], result["resources"])
    compiled_models: list[dict[str, object]] = cast(list[dict[str, object]], resources["models"])
    lineage: dict[str, object] = cast(dict[str, object], compiled_models[0]["lineage"])
    # order_id and the status alias each depend on order_id; total depends on
    # order_id, amount, and the physical status column, for five edges in total.
    assert lineage["edge_count"] == test_case.expected_edge_count
    compiled_sql: str = (
        tmp_path / "target" / "compiled" / "models" / "order_totals.sql"
    ).read_text()
    with duckdb.connect() as connection:
        connection.execute(
            "CREATE TABLE orders_input AS SELECT 1 AS order_id, "
            "CAST(10 AS DOUBLE) AS amount, 'priority' AS status"
        )
        # Compiled artifacts retain logical source intrinsics. Bind this test's
        # single source to its physical table before executing the artifact.
        connection.execute("CREATE MACRO __source(name) AS TABLE SELECT * FROM orders_input")
        assert connection.execute(compiled_sql).fetchall() == [(1, "odd", 20.0)]
        connection.execute("UPDATE orders_input SET status = 'standard'")
        assert connection.execute(compiled_sql).fetchall() == [(1, "odd", 9.0)]


@pytest.mark.parametrize(
    "test_case",
    (
        BoundProjectionCompileTestCase(
            description="qualified star exclusions preserve the output shape",
            query_sql='WITH selected AS (SELECT o.* EXCLUDE (category, amount) FROM __source("orders") o) SELECT * FROM selected ORDER BY order_id',
            expected_rows=((1,), (2,)),
            expected_edges=1,
        ),
        BoundProjectionCompileTestCase(
            description="positional CTE aliases retain unnamed nullable expressions",
            query_sql='WITH selected(a, b) AS (SELECT CAST(NULL AS VARCHAR), order_id FROM __source("orders")) SELECT a AS order_id FROM selected',
            expected_rows=((None,), (None,)),
            expected_edges=0,
            output_contract="order_id (type VARCHAR, nullable true)",
        ),
        BoundProjectionCompileTestCase(
            description="unnamed union branches retain every input dependency",
            query_sql='WITH selected AS (SELECT MAX(order_id) AS order_id FROM __source("orders") UNION ALL SELECT MAX(CAST(amount AS BIGINT)) FROM __source("orders")) SELECT order_id FROM selected ORDER BY order_id',
            expected_rows=((2,), (20,)),
            expected_edges=2,
            output_contract="order_id (type BIGINT, nullable true)",
        ),
        BoundProjectionCompileTestCase(
            description="named unions align inputs with different projection orders",
            query_sql='WITH selected AS (SELECT CAST(NULL AS BIGINT) AS order_id, amount FROM __source("orders") UNION ALL BY NAME SELECT amount, order_id FROM __source("orders")) SELECT order_id FROM selected ORDER BY order_id',
            expected_rows=((1,), (2,), (None,), (None,)),
            expected_edges=1,
            output_contract="order_id (type BIGINT, nullable true)",
        ),
        BoundProjectionCompileTestCase(
            description="value rows preserve positional aliases and nullable constants",
            query_sql='WITH selected AS (SELECT v.id AS order_id FROM (VALUES (CAST(1 AS BIGINT)), (CAST(NULL AS BIGINT))) AS v(id) CROSS JOIN __source("orders") AS o WHERE o.order_id = 1) SELECT order_id FROM selected ORDER BY order_id',
            expected_rows=((1,), (None,)),
            expected_edges=0,
            output_contract="order_id (type BIGINT, nullable true)",
        ),
        BoundProjectionCompileTestCase(
            description="star replacements preserve constant provenance",
            query_sql='WITH selected AS (SELECT * EXCLUDE (category, amount) REPLACE (CAST(0 AS BIGINT) AS order_id) FROM __source("orders")) SELECT * FROM selected',
            expected_rows=((0,), (0,)),
            expected_edges=0,
        ),
        BoundProjectionCompileTestCase(
            description="full using join coalesces its non-null input keys",
            query_sql='WITH joined AS (SELECT order_id FROM (SELECT order_id FROM __source("orders") WHERE order_id = 1) a FULL JOIN (SELECT order_id FROM __source("orders") WHERE order_id = 2) b USING (order_id)) SELECT order_id FROM joined ORDER BY order_id',
            expected_rows=((1,), (2,)),
            expected_edges=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_bound_wildcard_or_using_join_when_compiling_then_facts_match_execution(
    test_case: BoundProjectionCompileTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(_DERIVED_RELATION_PROJECT)
    sources: Path = tmp_path / "sources"
    sources.mkdir()
    (sources / "orders.yml").write_text(
        "sources:\n  - name: orders\n    description: Test source orders.\n    contract: enforced\n"
        "    expression: orders_input\n    columns:\n"
        "      - name: order_id\n        type: BIGINT\n        nullable: false\n"
        "      - name: category\n        type: VARCHAR\n"
        "      - name: amount\n        type: DOUBLE\n"
    )
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "selected_orders.sql").write_text(
        f"MODEL (description 'Test model selected_orders.', contract enforced, columns ({test_case.output_contract}));\n"
        + test_case.query_sql
    )
    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])
    result: dict[str, object] = json.loads(capsys.readouterr().out)
    assert exit_code == 0
    assert result["diagnostics"] == []
    resources: dict[str, object] = cast(dict[str, object], result["resources"])
    compiled_models: list[dict[str, object]] = cast(list[dict[str, object]], resources["models"])
    lineage: dict[str, object] = cast(dict[str, object], compiled_models[0]["lineage"])
    assert lineage["edge_count"] == test_case.expected_edges
    sql: str = (tmp_path / "target" / "compiled" / "models" / "selected_orders.sql").read_text()
    with duckdb.connect() as connection:
        connection.execute(
            "CREATE TABLE orders_input (order_id BIGINT, category VARCHAR, amount DOUBLE)"
        )
        connection.execute("INSERT INTO orders_input VALUES (1, 'ready', 10), (2, 'queued', 20)")
        connection.execute("CREATE MACRO __source(name) AS TABLE SELECT * FROM orders_input")
        assert tuple(connection.execute(sql).fetchall()) == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    (
        BoundProjectionCompileTestCase(
            description="object wildcard retains all possible payload inputs",
            query_sql='WITH packed AS (SELECT OBJECT_CONSTRUCT_KEEP_NULL(*) AS payload FROM __source("orders")) SELECT CAST(payload:order_id AS BIGINT) AS order_id FROM packed',
            expected_rows=(),
            expected_edges=2,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_object_wildcard_when_compiling_then_lineage_preserves_payload_inputs(
    test_case: BoundProjectionCompileTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "snowflake"\n[defaults]\ndatabase = "warehouse"\nschema = "analytics"\n'
    )
    sources: Path = tmp_path / "sources"
    sources.mkdir()
    (sources / "orders.yml").write_text(
        "sources:\n  - name: orders\n    description: Test source orders.\n    contract: enforced\n"
        "    expression: orders_input\n    columns:\n"
        "      - name: order_id\n        type: BIGINT\n"
        "      - name: amount\n        type: DOUBLE\n"
    )
    models: Path = tmp_path / "models"
    models.mkdir()
    (models / "selected_orders.sql").write_text(
        "MODEL (description 'Test model selected_orders.', contract enforced, columns (order_id (type BIGINT, nullable true)));\n"
        + test_case.query_sql
    )
    exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])
    result: dict[str, object] = json.loads(capsys.readouterr().out)
    assert exit_code == 0
    assert result["diagnostics"] == []
    resources: dict[str, object] = cast(dict[str, object], result["resources"])
    compiled_models: list[dict[str, object]] = cast(list[dict[str, object]], resources["models"])
    lineage: dict[str, object] = cast(dict[str, object], compiled_models[0]["lineage"])
    assert lineage["edge_count"] == test_case.expected_edges


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
