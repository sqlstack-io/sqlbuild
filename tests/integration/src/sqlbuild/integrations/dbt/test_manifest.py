from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

import pytest

from sqlbuild._native import (
    DbtCliOptions,
    DbtCommandResult,
    DbtRunner,
    DiscoveredProjectInputs,
    build_compile_inputs,
    discover_project_inputs,
)
from tests.integration.src.sqlbuild.integrations.dbt._test_types import (
    RealDbtManifestCompileTestCase,
)
from tests.integration.src.sqlbuild.integrations.dbt.helpers import (
    DUCKDB_COMPILE_ADAPTER_CONTEXT,
    build_external_sql_reference_resolver,
)

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView

pytestmark: pytest.MarkDecorator = pytest.mark.dbt


@pytest.mark.parametrize(
    "test_case",
    [
        RealDbtManifestCompileTestCase(
            description="validates and preserves SQLBuild dbt ref from real dbt manifest",
            sqlbuild_model_sql='MODEL (description "Test model.");\n\nselect order_id from __dbt_ref("stg_orders")\n',
            expected_compiled_sql='select order_id from __dbt_ref("stg_orders")',
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_real_dbt_manifest_when_compiling_sqlbuild_then_preserves_dbt_ref(
    test_case: RealDbtManifestCompileTestCase,
    real_dbt_executable: str,
    dbt_project_dir: Path,
    dbt_profiles_dir: Path,
    tmp_path: Path,
) -> None:
    options: DbtCliOptions = DbtCliOptions(
        project_dir=dbt_project_dir,
        profiles_dir=dbt_profiles_dir,
        target_path=dbt_project_dir / "target",
    )
    compile_result: DbtCommandResult = DbtRunner(dbt_executable=real_dbt_executable).compile(
        options=options
    )
    assert compile_result.returncode == 0, compile_result.stderr or compile_result.stdout

    sqlbuild_project_dir: Path = tmp_path / "sqlbuild_project"
    sqlbuild_project_dir.joinpath("sql", "models").mkdir(parents=True)
    sqlbuild_project_dir.joinpath("target").mkdir(parents=True)
    sqlbuild_project_dir.joinpath("sqlbuild_project.toml").write_text(
        'name = "demo"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    sqlbuild_project_dir.joinpath("sql/models/downstream_orders.sql").write_text(
        test_case.sqlbuild_model_sql, encoding="utf-8"
    )
    sqlbuild_project_dir.joinpath("target/manifest.json").write_text(
        dbt_project_dir.joinpath("target/manifest.json").read_text(encoding="utf-8"),
        encoding="utf-8",
    )

    manifest_source: Path = dbt_project_dir / "target/manifest.json"
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(
        project_dir=sqlbuild_project_dir
    )
    compile_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered_inputs,
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
        external_sql_reference_resolver=build_external_sql_reference_resolver(
            manifest_source=manifest_source
        ),
    )

    assert compile_inputs.model_inputs[0].query_sql == test_case.expected_compiled_sql
