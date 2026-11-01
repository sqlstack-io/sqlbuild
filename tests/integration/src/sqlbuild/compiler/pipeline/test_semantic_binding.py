"""Integration coverage for authoritative offline SQL column binding."""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from _pytest.capture import CaptureResult

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import (
    NumberWideningContractTestCase,
    SemanticBindingClauseIntegrationTestCase,
    SemanticBindingIntegrationTestCase,
    SourceContractDefaultIntegrationTestCase,
)
from tests.integration.src.sqlbuild.compiler.pipeline.helpers import write_semantic_binding_project

_AUTHORITATIVE_MODEL: str = """MODEL (description "Test model.",
  materialized table
  contract enforced
  columns (
    id (type INTEGER)
    category (type VARCHAR)
  )
);
SELECT CAST(1 AS INTEGER) AS id, CAST('a' AS VARCHAR) AS category
"""


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticBindingClauseIntegrationTestCase(
            description="where predicate",
            query_sql='SELECT id FROM __ref("upstream") WHERE missing_where = TRUE',
            missing_column="missing_where",
        ),
        SemanticBindingClauseIntegrationTestCase(
            description="join predicate",
            query_sql=(
                'SELECT l.id FROM __ref("upstream") l JOIN __ref("upstream") r '
                "ON l.id = r.missing_join"
            ),
            missing_column="missing_join",
        ),
        SemanticBindingClauseIntegrationTestCase(
            description="group by expression",
            query_sql=(
                'SELECT COUNT(*) AS count_rows FROM __ref("upstream") GROUP BY missing_group'
            ),
            missing_column="missing_group",
        ),
        SemanticBindingClauseIntegrationTestCase(
            description="having predicate",
            query_sql=(
                'SELECT category, COUNT(*) AS count_rows FROM __ref("upstream") '
                "GROUP BY category HAVING missing_having > 0"
            ),
            missing_column="missing_having",
        ),
        SemanticBindingClauseIntegrationTestCase(
            description="order by expression",
            query_sql='SELECT id FROM __ref("upstream") ORDER BY missing_order',
            missing_column="missing_order",
        ),
        SemanticBindingClauseIntegrationTestCase(
            description="window partition",
            query_sql=(
                "SELECT ROW_NUMBER() OVER (PARTITION BY missing_partition ORDER BY id) AS rn "
                'FROM __ref("upstream")'
            ),
            missing_column="missing_partition",
        ),
        SemanticBindingClauseIntegrationTestCase(
            description="window ordering",
            query_sql=(
                "SELECT ROW_NUMBER() OVER (PARTITION BY category ORDER BY missing_window_order) "
                'AS rn FROM __ref("upstream")'
            ),
            missing_column="missing_window_order",
        ),
        SemanticBindingClauseIntegrationTestCase(
            description="qualify predicate",
            query_sql=(
                "SELECT ROW_NUMBER() OVER (ORDER BY id) AS rn "
                'FROM __ref("upstream") QUALIFY missing_qualify = TRUE'
            ),
            missing_column="missing_qualify",
        ),
        SemanticBindingClauseIntegrationTestCase(
            description="join using",
            query_sql=(
                'SELECT l.id FROM __ref("upstream") l JOIN __ref("upstream") r '
                "USING (missing_using)"
            ),
            missing_column="missing_using",
        ),
        SemanticBindingClauseIntegrationTestCase(
            description="nested cte",
            query_sql=(
                'WITH scoped AS (SELECT id FROM __ref("upstream")) SELECT missing_cte FROM scoped'
            ),
            missing_column="missing_cte",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_complete_contract_when_clause_references_missing_column_then_compile_fails(
    test_case: SemanticBindingClauseIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_semantic_binding_project(
        project_dir=tmp_path,
        upstream_sql=_AUTHORITATIVE_MODEL,
        downstream_sql=f"MODEL (description 'Test model.', materialized view);\n{test_case.query_sql}\n",
    )

    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "compile", "--no-cache"])

    output: str = capsys.readouterr().out
    assert exit_code == test_case.expected_exit_code
    assert f"error[{test_case.expected_error_code}]" in output
    assert f"'{test_case.missing_column}'" in output
    assert "(context:" not in output
    assert "model: downstream" in output
    assert (
        f"--> models/downstream.sql:2:{test_case.query_sql.index(test_case.missing_column) + 1}"
        in output
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticBindingIntegrationTestCase(
            description="given two complete contracts when unqualified column is ambiguous then compile fails",
            expected_exit_code=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_two_complete_contracts_when_unqualified_column_is_ambiguous_then_compile_fails(
    test_case: SemanticBindingIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_semantic_binding_project(
        project_dir=tmp_path,
        upstream_sql=_AUTHORITATIVE_MODEL,
        downstream_sql=(
            "MODEL (description 'Test model.', materialized view);\n"
            'SELECT id FROM __ref("upstream") l CROSS JOIN __ref("other") r\n'
        ),
    )
    _ = (tmp_path / "models" / "other.sql").write_text(
        _AUTHORITATIVE_MODEL,
        encoding="utf-8",
    )

    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "compile", "--no-cache"])

    output: str = capsys.readouterr().out
    assert exit_code == test_case.expected_exit_code
    assert "error[B003]:" in output
    assert "id" in output


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticBindingIntegrationTestCase(
            description="explicit projection closes output names over an open input",
            expected_exit_code=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_explicit_projection_when_output_column_is_missing_then_compile_fails(
    test_case: SemanticBindingIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_semantic_binding_project(
        project_dir=tmp_path,
        upstream_sql="MODEL (description 'Test model.', materialized table);\nSELECT id FROM raw_orders\n",
        downstream_sql=(
            'MODEL (description "Test model.", materialized view);\nSELECT missing_column FROM __ref("upstream")\n'
        ),
    )

    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "compile", "--no-cache"])

    output: str = capsys.readouterr().out
    assert exit_code == test_case.expected_exit_code
    assert "error[B002]" in output


@pytest.mark.parametrize(
    "test_case",
    [
        SourceContractDefaultIntegrationTestCase(
            description="given default enforced source contract when column is missing then compile fails",
            source_contract_yaml="",
            expected_exit_code=1,
            expected_output_fragment="error[B002]: Unknown column 'missing' in raw_orders",
            expected_absent_output_fragment="0 errors, 0 warnings",
        ),
        SourceContractDefaultIntegrationTestCase(
            description="given explicit open source contract when default is enforced then compile succeeds",
            source_contract_yaml="    contract: none\n",
            expected_exit_code=0,
            expected_output_fragment="0 errors, 0 warnings",
            expected_absent_output_fragment="error[B002]",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_contract_default_when_compiling_then_effective_policy_controls_authority(
    test_case: SourceContractDefaultIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _ = (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "semantic_binding"\nadapter = "duckdb"\n\n[defaults]\ncontract = "enforced"\n',
        encoding="utf-8",
    )
    sources_dir: Path = tmp_path / "sources"
    models_dir: Path = tmp_path / "models"
    sources_dir.mkdir()
    models_dir.mkdir()
    _ = (sources_dir / "raw.yml").write_text(
        f"""sources:
  - name: raw_orders
    description: Test source raw_orders.
    schema: raw
    table: orders
{test_case.source_contract_yaml}    columns:
      - name: id
        type: INTEGER
""",
        encoding="utf-8",
    )
    _ = (models_dir / "downstream.sql").write_text(
        "MODEL (description 'Test model downstream.', materialized view, columns (missing (type INTEGER)));\n"
        'SELECT CAST(missing AS INTEGER) AS missing FROM __source("raw_orders")\n',
        encoding="utf-8",
    )

    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "compile", "--no-cache"])

    output: str = capsys.readouterr().out
    assert exit_code == test_case.expected_exit_code
    assert test_case.expected_output_fragment in output
    assert test_case.expected_absent_output_fragment not in output


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticBindingIntegrationTestCase(
            description="given complete inferred intermediate when downstream column is missing then compile fails",
            expected_exit_code=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_complete_inferred_intermediate_when_downstream_column_is_missing_then_compile_fails(
    test_case: SemanticBindingIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_semantic_binding_project(
        project_dir=tmp_path,
        upstream_sql=_AUTHORITATIVE_MODEL,
        downstream_sql=(
            "MODEL (description 'Test model.', materialized view);\n"
            'SELECT id FROM __ref("intermediate") WHERE missing_derived = TRUE\n'
        ),
    )
    _ = (tmp_path / "models" / "intermediate.sql").write_text(
        'MODEL (description "Test model intermediate.", materialized view);\nSELECT id FROM __ref("upstream")\n',
        encoding="utf-8",
    )

    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "compile", "--no-cache"])

    output: str = capsys.readouterr().out
    assert exit_code == test_case.expected_exit_code
    assert "error[B002]: Unknown column 'missing_derived'" in output


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticBindingIntegrationTestCase(
            description="given model analysis disabled when column is missing then binding is skipped",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_model_analysis_disabled_when_column_is_missing_then_binding_is_skipped(
    test_case: SemanticBindingIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_semantic_binding_project(
        project_dir=tmp_path,
        upstream_sql=_AUTHORITATIVE_MODEL,
        downstream_sql=(
            "MODEL (description 'Test model.',\n"
            "  materialized view\n"
            "  sql_analysis false\n"
            "  contract enforced\n"
            "  columns (\n"
            "    missing_vendor_column (type VARCHAR)\n"
            "    runtime_only_column (type VARCHAR)\n"
            "  )\n"
            ");\n"
            'SELECT missing_vendor_column FROM __ref("upstream")\n'
        ),
    )

    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "compile", "--no-cache"])

    output: str = capsys.readouterr().out
    assert exit_code == test_case.expected_exit_code
    assert "error[B" not in output
    assert "error[K" not in output


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticBindingIntegrationTestCase(
            description="given cached dependent error when upstream analysis is disabled then stale error clears",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_cached_dependent_error_when_upstream_analysis_disabled_then_stale_error_clears(
    test_case: SemanticBindingIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_semantic_binding_project(
        project_dir=tmp_path,
        upstream_sql=_AUTHORITATIVE_MODEL,
        downstream_sql=(
            'MODEL (description "Test model.", materialized view);\nSELECT runtime_column FROM __ref("intermediate")\n'
        ),
    )
    intermediate_path: Path = tmp_path / "models" / "intermediate.sql"
    _ = intermediate_path.write_text(
        'MODEL (description "Test model intermediate.", materialized view);\nSELECT id FROM __ref("upstream")\n',
        encoding="utf-8",
    )

    initial_exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "compile"])
    initial_output: str = capsys.readouterr().out
    assert initial_exit_code == 1
    assert "error[B002]: Unknown column 'runtime_column'" in initial_output

    _ = intermediate_path.write_text(
        "MODEL (description 'Test model.', materialized view, sql_analysis false);\n"
        'SELECT runtime_column FROM __ref("upstream")\n',
        encoding="utf-8",
    )
    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "compile"])
    output: str = capsys.readouterr().out

    assert exit_code == test_case.expected_exit_code
    assert "error[B002]" not in output

    cached_exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "compile"])
    cached_output: str = capsys.readouterr().out
    assert cached_exit_code == test_case.expected_exit_code
    assert "error[B002]" not in cached_output


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticBindingIntegrationTestCase(
            description="given cli analysis disabled when sql is unsupported then compile succeeds",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_cli_analysis_disabled_when_sql_is_unsupported_then_compile_succeeds(
    test_case: SemanticBindingIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _ = (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "semantic_binding"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    models_dir: Path = tmp_path / "models"
    models_dir.mkdir()
    _ = (models_dir / "vendor.sql").write_text(
        "MODEL (description 'Test model vendor.', materialized view);\nSELEC unsupported vendor sql\n",
        encoding="utf-8",
    )

    exit_code: int = main(
        [
            "--no-color",
            "--project-dir",
            str(tmp_path),
            "compile",
            "--no-sql-analysis",
        ]
    )

    _ = capsys.readouterr()
    assert exit_code == test_case.expected_exit_code


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticBindingIntegrationTestCase(
            description="given lineage uses requested when query filters then json keeps predicates separate",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_lineage_uses_requested_when_query_filters_then_json_keeps_predicates_separate(
    test_case: SemanticBindingIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_semantic_binding_project(
        project_dir=tmp_path,
        upstream_sql=_AUTHORITATIVE_MODEL,
        downstream_sql=(
            "MODEL (description 'Test model.', materialized view);\n"
            'SELECT id FROM __ref("upstream") u '
            "WHERE u.category = 'active' ORDER BY u.id\n"
        ),
    )

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "lineage",
            "downstream",
            "--format",
            "json",
            "--include-uses",
        ]
    )

    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert exit_code == test_case.expected_exit_code
    assert {(use["context"], use["source"]["column"]) for use in payload["semantic_uses"]} == {
        ("where", "category"),
        ("order_by", "id"),
    }
    assert all("semantic_uses" not in edge for edge in payload["edges"])

    default_exit_code: int = main(
        ["--project-dir", str(tmp_path), "lineage", "downstream", "--format", "json"]
    )
    default_payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert default_exit_code == 0
    assert "semantic_uses" not in default_payload


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticBindingIntegrationTestCase(
            description="given valid using and qualify alias when binding then compile succeeds",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_valid_using_and_qualify_alias_when_binding_then_compile_succeeds(
    test_case: SemanticBindingIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_semantic_binding_project(
        project_dir=tmp_path,
        upstream_sql=_AUTHORITATIVE_MODEL,
        downstream_sql=(
            "MODEL (description 'Test model.', materialized view);\n"
            "SELECT ROW_NUMBER() OVER (ORDER BY l.id) AS rn "
            'FROM __ref("upstream") l JOIN __ref("upstream") r USING (id) '
            "QUALIFY rn = 1\n"
        ),
    )

    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "compile", "--no-cache"])

    output: str = capsys.readouterr().out
    assert exit_code == test_case.expected_exit_code
    assert "error[B" not in output


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticBindingIntegrationTestCase(
            description="given unresolved star when contract has additional column then absence is uncertain",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unresolved_star_when_contract_has_additional_column_then_compile_succeeds(
    test_case: SemanticBindingIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_semantic_binding_project(
        project_dir=tmp_path,
        upstream_sql=_AUTHORITATIVE_MODEL,
        downstream_sql=(
            "MODEL (description 'Test model.',\n"
            "  materialized view\n"
            "  contract enforced\n"
            "  columns (\n"
            "    id (type INTEGER)\n"
            "    category (type VARCHAR)\n"
            "    runtime_column (type VARCHAR)\n"
            "  )\n"
            ");\n"
            'SELECT * FROM __ref("upstream")\n'
        ),
    )

    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "compile", "--no-cache"])

    output: str = capsys.readouterr().out
    assert exit_code == test_case.expected_exit_code
    assert "error[K001]" not in output


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticBindingIntegrationTestCase(
            description="varchar cast length remains available through a CTE",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sized_varchar_cast_when_compiling_enforced_contract_then_type_matches(
    test_case: SemanticBindingIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_semantic_binding_project(
        project_dir=tmp_path,
        upstream_sql=_AUTHORITATIVE_MODEL,
        downstream_sql=(
            "MODEL (description 'Test model.',\n"
            "  materialized view\n"
            "  contract enforced\n"
            "  columns (category (type VARCHAR(3)))\n"
            ");\n"
            "WITH final AS (\n"
            '  SELECT CAST(category AS VARCHAR(3)) AS category FROM __ref("upstream")\n'
            ")\n"
            "SELECT final.category FROM final\n"
        ),
    )

    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "compile", "--no-cache"])

    output: str = capsys.readouterr().out
    assert exit_code == test_case.expected_exit_code
    assert "error[K002]" not in output
    assert "error[K003]" not in output


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticBindingIntegrationTestCase(
            description="declared cast type survives CTEs when source columns are untyped",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_untyped_source_when_cast_flows_through_ctes_then_declared_type_matches(
    test_case: SemanticBindingIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _ = (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "semantic_binding"\nadapter = "snowflake"\n', encoding="utf-8"
    )
    sources_dir: Path = tmp_path / "sources"
    models_dir: Path = tmp_path / "models"
    sources_dir.mkdir()
    models_dir.mkdir()
    _ = (sources_dir / "raw.yml").write_text(
        """sources:
  - name: raw_orders
    description: Test source raw_orders.
    schema: raw
    table: orders
    columns:
      - name: payload
""",
        encoding="utf-8",
    )
    model_path: Path = models_dir / "orders.sql"
    model_sql: str = """MODEL (description "Test model orders.",
  materialized view
  database analytics
  schema analytics
);
WITH raw_orders AS (
  SELECT PARSE_JSON(payload) AS document FROM __source("raw_orders")
),
latest_orders AS (
  SELECT document:data:categories AS categories
  FROM raw_orders
  QUALIFY ROW_NUMBER() OVER (ORDER BY document:updated_at::TIMESTAMP DESC) = 1
),
flattened AS (
  SELECT item.value::VARCHAR AS category
  FROM latest_orders, LATERAL FLATTEN(input => categories) item
),
untyped AS (
  SELECT category FROM flattened
),
final AS (
  SELECT CAST(category AS VARCHAR(3)) AS category FROM untyped
)
SELECT category FROM final
"""
    _ = model_path.write_text(model_sql, encoding="utf-8")

    initial_exit_code: int = main(
        ["--no-color", "--project-dir", str(tmp_path), "compile", "--json"]
    )
    initial_output: dict[str, object] = json.loads(capsys.readouterr().out)
    assert initial_exit_code == test_case.expected_exit_code
    assert isinstance(initial_output.get("diagnostics"), list)

    _ = model_path.write_text(
        model_sql.replace(
            "schema analytics\n",
            "schema analytics\n  columns (category (type VARCHAR(3)))\n",
        ),
        encoding="utf-8",
    )

    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "compile", "--json"])

    captured: CaptureResult[str] = capsys.readouterr()
    output: dict[str, object] = json.loads(captured.out)
    diagnostics: object = output.get("diagnostics")
    assert exit_code == test_case.expected_exit_code
    assert isinstance(diagnostics, list)
    assert not any(
        isinstance(diagnostic, dict) and diagnostic.get("code") in {"K002", "K003"}
        for diagnostic in diagnostics
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticBindingIntegrationTestCase(
            description="safe expression and union types survive CTE passthroughs",
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_typed_union_ctes_when_compiling_enforced_contract_then_types_are_proven(
    test_case: SemanticBindingIntegrationTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_semantic_binding_project(
        project_dir=tmp_path,
        upstream_sql=_AUTHORITATIVE_MODEL.replace(
            "  materialized table\n",
            "  materialized table\n  database analytics\n  schema analytics\n",
        ),
        downstream_sql=(
            "MODEL (description 'Test model.',\n"
            "  materialized view\n"
            "  database analytics\n"
            "  schema analytics\n"
            "  contract enforced\n"
            "  columns (\n"
            "    category (type VARCHAR)\n"
            "    is_selected (type BOOLEAN)\n"
            "    processed_at (type TIMESTAMP_NTZ)\n"
            "    notes (type VARCHAR)\n"
            "  )\n"
            ");\n"
            "WITH current_orders AS (\n"
            "  SELECT CAST(category AS VARCHAR) AS category,\n"
            "    id IN (1, 2) AS is_selected,\n"
            "    TO_TIMESTAMP(0) AS processed_at,\n"
            "    CAST(category AS VARCHAR) AS notes\n"
            '  FROM __ref("upstream")\n'
            "), combined AS (\n"
            "  SELECT category, is_selected, processed_at, notes FROM current_orders\n"
            "  -- Preserve archived records.\n"
            "  UNION ALL\n"
            "  SELECT category, TRUE AS is_selected, CAST(NULL AS TIMESTAMP) AS processed_at,\n"
            '    NULL AS notes FROM __ref("upstream")\n'
            "), final AS (\n"
            "  SELECT category, is_selected, processed_at, notes FROM combined\n"
            ")\n"
            "SELECT category, is_selected, processed_at, notes FROM final\n"
        ),
    )
    _ = (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "semantic_binding"\nadapter = "snowflake"\n',
        encoding="utf-8",
    )

    exit_code: int = main(
        ["--no-color", "--project-dir", str(tmp_path), "compile", "--no-cache", "--json"]
    )

    output: dict[str, object] = json.loads(capsys.readouterr().out)
    diagnostics: object = output.get("diagnostics")
    assert exit_code == test_case.expected_exit_code
    assert isinstance(diagnostics, list)
    assert not any(
        isinstance(diagnostic, dict) and diagnostic.get("code") in {"K002", "K003"}
        for diagnostic in diagnostics
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])


@pytest.mark.parametrize(
    "test_case",
    [
        NumberWideningContractTestCase(
            description="a windowed COUNT(DISTINCT) fits a declared NUMBER(38,0)",
            declared_type="NUMBER(38,0)",
            expected_exit_code=0,
            expected_type_mismatch=False,
        ),
        NumberWideningContractTestCase(
            description="a declared NUMBER(10,0) is narrower than the COUNT",
            declared_type="NUMBER(10,0)",
            expected_exit_code=1,
            expected_type_mismatch=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_count_when_compiling_enforced_contract_then_lossless_widening_is_accepted(
    test_case: NumberWideningContractTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _ = (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "snowflake"\n\n[rules]\nselect = ["SQBRCONTRACT105"]\n',
        encoding="utf-8",
    )
    models: Path = tmp_path / "models"
    models.mkdir()
    _ = (models / "orders.sql").write_text(
        "MODEL (description 'Test model.', database warehouse, schema analytics,\n"
        "  materialized table\n"
        "  contract enforced\n"
        "  columns (customer_id (type VARCHAR), status (type VARCHAR))\n"
        ");\n"
        "SELECT CAST('c' AS VARCHAR) AS customer_id, CAST('open' AS VARCHAR) AS status\n",
        encoding="utf-8",
    )
    _ = (models / "order_counts.sql").write_text(
        "MODEL (description 'Test model.', database warehouse, schema analytics,\n"
        "  materialized table\n"
        "  contract enforced\n"
        f"  columns (customer_id (type VARCHAR), statuses (type {test_case.declared_type}))\n"
        ");\n"
        "WITH counted AS (\n"
        "  SELECT customer_id,\n"
        "    COUNT(DISTINCT status) OVER (PARTITION BY customer_id) AS statuses\n"
        '  FROM __ref("orders")\n'
        "),\n"
        "final AS (SELECT customer_id, statuses FROM counted)\n"
        "SELECT customer_id, statuses FROM final\n",
        encoding="utf-8",
    )

    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "compile", "--no-cache"])

    output: str = capsys.readouterr().out
    assert exit_code == test_case.expected_exit_code, output
    assert ("error[K002]" in output) == test_case.expected_type_mismatch, output
    assert "SQBRCONTRACT105" not in output or test_case.expected_type_mismatch, output
