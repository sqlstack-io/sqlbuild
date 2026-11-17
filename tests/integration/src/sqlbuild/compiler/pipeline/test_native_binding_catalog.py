"""Native catalog semantics through the compiler and physical rebinding path."""

import json
from pathlib import Path
from typing import Any

import pytest

import sqlbuild._native as sql_analysis_models
from sqlbuild._native import (
    CompiledObjectKey,
    CompilerDiagnostic,
    build_compiled_project,
    discover_project_inputs,
    source_binding_diagnostics,
)
from sqlbuild._native import run_cli as main
from sqlbuild.adapter.contract.models import ColumnInfo, ExpressionInferenceProfile
from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import (
    NativeCatalogCase,
    PhysicalEvidenceCase,
    ValidationCacheCase,
)

_DECLARED_ORDERS: tuple[tuple[str, str], ...] = (("order_id", "INTEGER"), ("status", "VARCHAR"))


@pytest.mark.parametrize(
    "test_case",
    [
        NativeCatalogCase("derived alias", 'SELECT "orderid" FROM (SELECT 1 AS "OrderId") q'),
        NativeCatalogCase(
            "CTE and alias",
            'WITH "OrderValues" AS (SELECT 1 AS "OrderId") SELECT "orderid" FROM "ordervalues"',
        ),
        NativeCatalogCase(
            "binding preserves type checks",
            'SELECT SUM("active") FROM (SELECT TRUE AS "Active") q',
            ("B213",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_quoted_case_semantics_when_compiling_then_resolves_native_aliases(
    test_case: NativeCatalogCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "snowflake"\n[rules]\nselect = []\n[connection]\nsession_parameters = { QUOTED_IDENTIFIERS_IGNORE_CASE = true }\n'
    )
    (tmp_path / "models").mkdir()
    (tmp_path / "models/orders.sql").write_text(
        "MODEL (description 'Test model orders.', database warehouse, schema analytics);\n"
        + test_case.sql
    )
    main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"])
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert tuple(item["code"] for item in payload["diagnostics"]) == test_case.expected_codes


@pytest.mark.parametrize(
    "test_case",
    [
        NativeCatalogCase(
            "authored quoted typo", 'SELECT "MiSsPeLlEd" FROM (SELECT 1 AS "OrderId") q', ("B002",)
        ),
        NativeCatalogCase(
            "uppercase quoted typo",
            'SELECT "MISSPELLED" FROM (SELECT 1 AS "OrderId") q',
            ("B002",),
            "MISSPELLED",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_quoted_typo_when_validating_then_keeps_authored_spelling_and_span(
    test_case: NativeCatalogCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "snowflake"\n[rules]\nselect = []\n[connection]\nsession_parameters = { QUOTED_IDENTIFIERS_IGNORE_CASE = true }\n'
    )
    (tmp_path / "models").mkdir()
    sql: str = (
        "MODEL (description 'Test model.', database warehouse, schema analytics);\n" + test_case.sql
    )
    (tmp_path / "models/orders.sql").write_text(sql)
    assert main(["--project-dir", str(tmp_path), "compile", "--json", "--no-cache"]) == 1
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    (diagnostic,) = payload["diagnostics"]
    assert (diagnostic["code"],) == test_case.expected_codes
    assert test_case.spelling in diagnostic["message"]
    assert diagnostic["line"] == 2
    assert f'"{test_case.spelling}"' in payload["resources"]["models"][0]["query_sql"]


@pytest.mark.parametrize(
    "test_case",
    [
        NativeCatalogCase(
            "physical schema refresh", 'SELECT "orderid" FROM __source("orders")', ("B002",)
        )
    ],
    ids=lambda case: case.description,
)
def test_given_catalog_when_rebinding_physical_shapes_then_does_not_reuse_stale_columns(
    test_case: NativeCatalogCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "snowflake"\n[rules]\nselect = []\n[connection]\nsession_parameters = { QUOTED_IDENTIFIERS_IGNORE_CASE = true }\n'
    )
    (tmp_path / "models").mkdir()
    (tmp_path / "sources").mkdir()
    (tmp_path / "sources/orders.yml").write_text(
        "sources:\n  - name: orders\n    description: Test source orders.\n    table: orders\n"
    )
    (tmp_path / "models/report.sql").write_text(
        "MODEL (description 'Test model report.', database warehouse, schema analytics);\n"
        + test_case.sql
    )
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path), adapter=SnowflakeAdapter()
    )
    assert project.binding_catalog is not None
    keys: frozenset[CompiledObjectKey] = frozenset(model.key for model in project.models)
    profile: ExpressionInferenceProfile = ExpressionInferenceProfile(
        sql_analysis_dialect="snowflake"
    )
    missing: tuple[CompilerDiagnostic, ...] = source_binding_diagnostics(
        project=project,
        columns={"orders": (ColumnInfo("customer", "VARCHAR"),)},
        profile=profile,
        selected_keys=keys,
    )
    present: tuple[CompilerDiagnostic, ...] = source_binding_diagnostics(
        project=project,
        columns={"orders": (ColumnInfo("OrderId", "INTEGER"),)},
        profile=profile,
        selected_keys=keys,
    )
    repeated: tuple[CompilerDiagnostic, ...] = source_binding_diagnostics(
        project=project,
        columns={"orders": (ColumnInfo("customer", "VARCHAR"),)},
        profile=profile,
        selected_keys=keys,
    )
    assert tuple(item.code for item in missing) == test_case.expected_codes
    assert present == ()
    assert repeated == missing


@pytest.mark.parametrize(
    "test_case",
    [
        NativeCatalogCase("unquoted reference", 'SELECT bundle_sku FROM __source("orders")'),
        NativeCatalogCase("uppercase reference", 'SELECT BUNDLE_SKU FROM __source("orders")'),
        NativeCatalogCase(
            "quoted uppercase reference", 'SELECT "BUNDLE_SKU" FROM __source("orders")'
        ),
        NativeCatalogCase(
            "quoted lowercase reference", 'SELECT "bundle_sku" FROM __source("orders")', ("B002",)
        ),
        NativeCatalogCase(
            "unknown column", 'SELECT missing_sku FROM __source("orders")', ("B002",)
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_case_sensitive_snowflake_when_rebinding_listed_columns_then_unquoted_names_bind(
    test_case: NativeCatalogCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "snowflake"\n[rules]\nselect = []\n'
    )
    (tmp_path / "models").mkdir()
    (tmp_path / "sources").mkdir()
    (tmp_path / "sources/orders.yml").write_text(
        "sources:\n  - name: orders\n    description: Test source orders.\n    table: orders\n"
    )
    (tmp_path / "models/report.sql").write_text(
        "MODEL (description 'Test model report.', database warehouse, schema analytics);\n"
        + test_case.sql
    )
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path), adapter=SnowflakeAdapter()
    )
    diagnostics: tuple[CompilerDiagnostic, ...] = source_binding_diagnostics(
        project=project,
        columns={"orders": (ColumnInfo("bundle_sku", "VARCHAR"),)},
        profile=ExpressionInferenceProfile(sql_analysis_dialect="snowflake"),
        selected_keys=frozenset(model.key for model in project.models),
    )
    assert tuple(item.code for item in diagnostics) == test_case.expected_codes


@pytest.mark.parametrize(
    "test_case",
    [
        PhysicalEvidenceCase(
            "Snowflake listed names equal to declared names",
            SnowflakeAdapter,
            "snowflake",
            _DECLARED_ORDERS,
            expected_rebinding=False,
        ),
        PhysicalEvidenceCase(
            "Snowflake new listed column",
            SnowflakeAdapter,
            "snowflake",
            (*_DECLARED_ORDERS, ("region", "VARCHAR")),
            expected_rebinding=True,
        ),
        PhysicalEvidenceCase(
            "Snowflake lowercase stored name rejects unquoted reader",
            SnowflakeAdapter,
            "snowflake",
            (('"order_id"', "INTEGER"), ("status", "VARCHAR")),
            expected_rebinding=True,
            expected_codes=("B002",),
        ),
        PhysicalEvidenceCase(
            "Snowflake default length and precision spellings of the declared types",
            SnowflakeAdapter,
            "snowflake",
            (("order_id", "NUMBER(38,0)"), ("status", "VARCHAR(16777216)")),
            expected_rebinding=False,
        ),
        PhysicalEvidenceCase(
            "Snowflake shorter listed VARCHAR",
            SnowflakeAdapter,
            "snowflake",
            (("order_id", "NUMBER(38,0)"), ("status", "VARCHAR(10)")),
            expected_rebinding=True,
        ),
        PhysicalEvidenceCase(
            "Snowflake changed column type",
            SnowflakeAdapter,
            "snowflake",
            (("order_id", "VARCHAR"), ("status", "VARCHAR")),
            expected_rebinding=True,
        ),
        PhysicalEvidenceCase(
            "Postgres listed names equal to declared names",
            PostgresAdapter,
            "postgres",
            _DECLARED_ORDERS,
            expected_rebinding=False,
        ),
        PhysicalEvidenceCase(
            "Postgres mixed-case stored name rejects unquoted reader",
            PostgresAdapter,
            "postgres",
            (("Order_Id", "INTEGER"), ("status", "VARCHAR")),
            expected_rebinding=True,
            expected_codes=("B002",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_case_sensitive_dialect_when_listed_columns_bind_as_declared_then_skips_rebinding(
    test_case: PhysicalEvidenceCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "orders"\nadapter = "{test_case.dialect}"\n[rules]\nselect = []\n'
    )
    (tmp_path / "models").mkdir()
    (tmp_path / "sources").mkdir()
    (tmp_path / "sources/orders.yml").write_text(
        "sources:\n  - name: orders\n    description: Test source orders.\n    table: orders\n"
        "    contract: enforced\n    columns:\n"
        + "".join(
            f"      - name: {name}\n        type: {kind}\n" for name, kind in _DECLARED_ORDERS
        )
    )
    (tmp_path / "models/report.sql").write_text(
        "MODEL (description 'Test model report.', database warehouse, schema analytics);\n"
        'SELECT order_id, status FROM __source("orders")'
    )
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path), adapter=test_case.adapter()
    )
    _ = sql_analysis_models.compile_stage_work_info(reset=True)

    diagnostics: tuple[CompilerDiagnostic, ...] = source_binding_diagnostics(
        project=project,
        columns={"orders": tuple(ColumnInfo(name, kind) for name, kind in test_case.listed)},
        profile=ExpressionInferenceProfile(sql_analysis_dialect=test_case.dialect),
        selected_keys=frozenset(model.key for model in project.models),
    )

    rebinding: int = sql_analysis_models.compile_stage_work_info().get(
        "source_rebinding_analyses", 0
    )
    assert (rebinding > 0) is test_case.expected_rebinding
    assert tuple(item.code for item in diagnostics) == test_case.expected_codes


@pytest.mark.parametrize(
    "test_case",
    [
        NativeCatalogCase(
            "unquoted type error with quoted-case policy", "SELECT SUM(TRUE) AS total", ("B213",)
        ),
        NativeCatalogCase(
            "quoted type error with quoted-case policy", 'SELECT SUM(TRUE) AS "total"', ("B213",)
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_when_case_policy_enabled_then_fused_validation_is_reused(
    test_case: NativeCatalogCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "snowflake"\n[rules]\nselect = []\n'
        "[connection]\nsession_parameters = { QUOTED_IDENTIFIERS_IGNORE_CASE = true }\n"
    )
    (tmp_path / "models").mkdir()
    (tmp_path / "models/orders.sql").write_text(
        "MODEL (description 'Test model orders.', database warehouse, schema analytics);\n"
        + test_case.sql
    )

    def repeated_binding(**kwargs: Any) -> None:
        raise AssertionError("already validated SQL was normalized and bound again")

    monkeypatch.setattr(sql_analysis_models, "SqlSchemaValidationRequest", repeated_binding)
    assert (
        main(
            [
                "--project-dir",
                str(tmp_path),
                "compile",
                "--lineage-mode",
                "rich",
                "--no-cache",
                "--json",
            ]
        )
        == 1
    )
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert tuple(item["code"] for item in payload["diagnostics"]) == test_case.expected_codes


@pytest.mark.parametrize(
    "test_case",
    [
        ValidationCacheCase("enabled target reuses semantic failures", True, 1, 0),
        ValidationCacheCase("disabled target revalidates unchanged SQL", False, 0, 1),
    ],
    ids=lambda case: case.description,
)
def test_given_target_cache_policy_when_compiling_twice_then_reuses_only_enabled_validation(
    test_case: ValidationCacheCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "snowflake"\ndefault_target = "local"\n'
        '[rules]\nselect = []\n[targets.local]\nschema = "preserve"\n'
        f"compile_cache = {str(test_case.enabled).lower()}\n"
        "[targets.local.connection]\nsession_parameters = { QUOTED_IDENTIFIERS_IGNORE_CASE = true }\n"
    )
    (tmp_path / "models").mkdir()
    model: Path = tmp_path / "models/orders.sql"
    model.write_text(
        'MODEL (description "Test model orders.", database warehouse, schema analytics);\nSELECT SUM("Active") FROM (SELECT TRUE AS "Active") q'
    )
    args: list[str] = [
        "--project-dir",
        str(tmp_path),
        "compile",
        "--lineage-mode",
        "rich",
        "--json",
    ]
    assert main(args) == 1
    cold: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert main(args) == 1
    warm: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert [item["code"] for item in cold["diagnostics"]] == ["B213"]
    assert warm["diagnostics"] == cold["diagnostics"]
    assert (
        warm["compile_timings"]["analysis_batch_cache_hits"]
        + warm["compile_timings"]["analysis_entry_cache_hits"]
    ) == test_case.expected_hits
    assert warm["compile_timings"]["analysis_cache_bypasses"] == test_case.expected_bypasses
    assert warm["compile_timings"]["analysis_cache_misses"] == 0
    model.write_text(model.read_text().replace("TRUE", "1"))
    assert main(args) == 0
    changed: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert changed["diagnostics"] == []
    assert changed["compile_timings"]["analysis_batch_cache_hits"] == 0


@pytest.mark.parametrize(
    "test_case",
    [
        NativeCatalogCase(
            "quoted bound CTE lineage",
            'WITH q AS (SELECT id, CASE WHEN id > 0 THEN amount ELSE 0 END AS total FROM __source("raw_orders")) SELECT id AS "ORDER_ID", total FROM q',
        )
    ],
    ids=lambda case: case.description,
)
def test_given_quoted_catalog_binding_when_compiling_then_preserves_bound_expression_lineage(
    test_case: NativeCatalogCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "snowflake"\n[rules]\nselect = []\n'
        "[connection]\nsession_parameters = { QUOTED_IDENTIFIERS_IGNORE_CASE = true }\n"
    )
    (tmp_path / "sources").mkdir()
    (tmp_path / "sources/orders.yml").write_text(
        "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n    expression: SELECT 1 AS id, 2 AS amount\n"
    )
    (tmp_path / "models").mkdir()
    (tmp_path / "models/orders.sql").write_text(
        "MODEL (description 'Test model orders.', database warehouse, schema analytics); "
        + test_case.sql
    )
    assert (
        main(
            [
                "--project-dir",
                str(tmp_path),
                "compile",
                "--lineage-mode",
                "rich",
                "--no-cache",
                "--json",
            ]
        )
        == 0
    )
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert tuple(item["code"] for item in payload["diagnostics"]) == test_case.expected_codes
    (model,) = payload["resources"]["models"]
    assert model["column_count"] == 2
    assert model["lineage"]["edge_count"] == 3


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
