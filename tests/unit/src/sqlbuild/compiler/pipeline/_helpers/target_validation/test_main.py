from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    CompiledRelationLocation,
    DiscoveredProjectInputs,
    DiscoveredSourceFile,
    LocalConfig,
    LocalTargetConfig,
    ProjectConfig,
    SourceEntry,
    TargetConfig,
    validate_managed_loader_target_isolation,
    validate_managed_write_schemas,
    validate_named_target_schema_strategy,
    validate_project_targets,
)
from sqlbuild.adapter.contract.types import BuiltinAdapter
from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from sqlbuild.adapters.databricks.classes.databricks_adapter import DatabricksAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.motherduck.classes.motherduck_adapter import MotherDuckAdapter
from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.adapters.sqlserver.classes.sqlserver_adapter import SqlServerAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.unit.src.sqlbuild.compiler.pipeline._helpers.target_validation._test_types import (
    ValidateManagedLoaderConnectionTestCase,
    ValidateManagedWriteSchemaTestCase,
    ValidateNamedTargetSchemaTestCase,
    ValidateProjectTargetsTestCase,
)
from tests.unit.src.sqlbuild.compiler.pipeline._helpers.target_validation.helpers import (
    ConservativeCustomAdapter,
    build_project,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ValidateManagedLoaderConnectionTestCase(
            description="resolved destination connection bypasses origin connection interpolation",
            resolved_connection={"database": "destination"},
            expected_schemas=("dev_loader", "prod_loader"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_resolved_clone_connection_when_validating_loaders_then_origin_credentials_are_unused(
    test_case: ValidateManagedLoaderConnectionTestCase,
) -> None:
    discovered_inputs: DiscoveredProjectInputs = DiscoveredProjectInputs(
        project_config=ProjectConfig(
            name="test",
            adapter="postgres",
            targets={
                "dev": TargetConfig(schema="dev", loader_schema=test_case.expected_schemas[0]),
                "prod": TargetConfig(
                    schema="prod",
                    loader_schema=test_case.expected_schemas[1],
                    connection={"password": "${ENV:MISSING_ORIGIN_PASSWORD}"},
                ),
            },
        ),
        local_config=LocalConfig(),
        source_files=(
            DiscoveredSourceFile(
                file_path=Path("sources/managed.yml"),
                relative_path=Path("sources/managed.yml"),
                contents="",
                source_entries=(SourceEntry(name="managed", managed=True),),
            ),
        ),
    )

    validate_managed_loader_target_isolation(
        discovered_inputs=discovered_inputs,
        adapter=PostgresAdapter(),
        resolved_connection=test_case.resolved_connection,
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ValidateProjectTargetsTestCase(
            description="snowflake requires explicit database and schema",
            adapter_name=BuiltinAdapter.SNOWFLAKE,
            target=CompiledRelationLocation(
                database=None, schema=None, name="stg_customers", qualified_name=None
            ),
            expected_error_fragment="snowflake execution requires explicit target database, schema",
        ),
        ValidateProjectTargetsTestCase(
            description="bigquery requires explicit database and schema",
            adapter_name=BuiltinAdapter.BIGQUERY,
            target=CompiledRelationLocation(
                database=None, schema=None, name="stg_customers", qualified_name=None
            ),
            expected_error_fragment="bigquery execution requires explicit target database, schema",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_adapter_with_missing_target_namespace_when_validating_then_raises_clear_error(
    test_case: ValidateProjectTargetsTestCase,
) -> None:
    project: CompiledProjectView = build_project(target=test_case.target, effective_connection={})

    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        validate_project_targets(adapter_name=test_case.adapter_name, project=project)


@pytest.mark.parametrize(
    "test_case",
    [
        ValidateProjectTargetsTestCase(
            description="duckdb allows missing database and schema",
            adapter_name=BuiltinAdapter.DUCKDB,
            target=CompiledRelationLocation(
                database=None,
                schema=None,
                name="stg_customers",
                qualified_name=None,
            ),
            expected_error_fragment="",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_duckdb_with_missing_target_namespace_when_validating_then_it_passes(
    test_case: ValidateProjectTargetsTestCase,
) -> None:
    project: CompiledProjectView = build_project(target=test_case.target, effective_connection={})

    validate_project_targets(adapter_name=test_case.adapter_name, project=project)
    assert test_case.expected_error_fragment == ""


@pytest.mark.parametrize(
    "test_case",
    (
        ValidateManagedWriteSchemaTestCase(
            description="Postgres adapter default does not satisfy write policy",
            adapter=PostgresAdapter(),
            target_schema=None,
            effective_connection={},
            expected_error_fragment="Model 'stg_customers' has no explicitly resolved",
        ),
        ValidateManagedWriteSchemaTestCase(
            description="MotherDuck adapter default does not satisfy write policy",
            adapter=MotherDuckAdapter(),
            target_schema=None,
            effective_connection={},
            expected_error_fragment="Model 'stg_customers' has no explicitly resolved",
        ),
        ValidateManagedWriteSchemaTestCase(
            description="Snowflake rejects an implicit write schema",
            adapter=SnowflakeAdapter(),
            target_schema=None,
            effective_connection={},
            expected_error_fragment="Model 'stg_customers' has no explicitly resolved",
        ),
        ValidateManagedWriteSchemaTestCase(
            description="BigQuery rejects an implicit write schema",
            adapter=BigQueryAdapter(),
            target_schema=None,
            effective_connection={},
            expected_error_fragment="Model 'stg_customers' has no explicitly resolved",
        ),
        ValidateManagedWriteSchemaTestCase(
            description="Databricks rejects an implicit write schema",
            adapter=DatabricksAdapter(),
            target_schema=None,
            effective_connection={},
            expected_error_fragment="Model 'stg_customers' has no explicitly resolved",
        ),
        ValidateManagedWriteSchemaTestCase(
            description="SQL Server default does not satisfy write policy",
            adapter=SqlServerAdapter(),
            target_schema=None,
            effective_connection={},
            expected_error_fragment="Model 'stg_customers' has no explicitly resolved",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_warehouse_managed_write_without_schema_when_validating_then_error_is_raised(
    test_case: ValidateManagedWriteSchemaTestCase,
) -> None:
    project: CompiledProjectView = build_project(
        target=CompiledRelationLocation(
            database=None,
            schema=test_case.target_schema,
            name="stg_customers",
            qualified_name=None,
        ),
        effective_connection=test_case.effective_connection,
    )

    with pytest.raises(ValueError, match=str(test_case.expected_error_fragment)):
        validate_managed_write_schemas(adapter=test_case.adapter, project=project)


@pytest.mark.parametrize(
    "test_case",
    (
        ValidateManagedWriteSchemaTestCase(
            description="literal resolved schema satisfies write policy",
            adapter=PostgresAdapter(),
            target_schema="analytics",
            effective_connection={},
            expected_error_fragment=None,
        ),
        ValidateManagedWriteSchemaTestCase(
            description="an inline connection's schema, as the project schema, satisfies write policy",
            adapter=PostgresAdapter(),
            target_schema=None,
            effective_connection={"schema": "analytics"},
            effective_target_schema="analytics",
            expected_error_fragment=None,
        ),
        ValidateManagedWriteSchemaTestCase(
            description="DuckDB permits implicit main schema",
            adapter=DuckDbAdapter(),
            target_schema=None,
            effective_connection={},
            expected_error_fragment=None,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_explicit_or_permitted_schema_when_validating_managed_write_then_it_passes(
    test_case: ValidateManagedWriteSchemaTestCase,
) -> None:
    project: CompiledProjectView = build_project(
        target=CompiledRelationLocation(
            database=None,
            schema=test_case.target_schema,
            name="stg_customers",
            qualified_name=None,
        ),
        effective_connection=test_case.effective_connection,
        effective_target_schema=test_case.effective_target_schema,
    )

    validate_managed_write_schemas(adapter=test_case.adapter, project=project)
    assert test_case.expected_error_fragment is None


@pytest.mark.parametrize(
    "test_case",
    (
        ValidateNamedTargetSchemaTestCase(
            description="shared warehouse named target omits schema",
            adapter=PostgresAdapter(),
            project_config=ProjectConfig(
                name="test",
                adapter="postgres",
                default_target="dev",
                targets={"dev": TargetConfig()},
            ),
            local_config=LocalConfig(),
            selected_target=None,
            expected_error_fragment=(
                "Named target 'dev' must explicitly set schema to a nonblank literal or 'preserve'"
            ),
        ),
        ValidateNamedTargetSchemaTestCase(
            description="named connection schema does not replace target namespace strategy",
            adapter=PostgresAdapter(),
            project_config=ProjectConfig(
                name="test",
                adapter="postgres",
                default_target="dev",
                connections={"developer": {"database": "ANALYTICS", "schema": "DEV_USER"}},
                targets={"dev": TargetConfig(connection_name="developer")},
            ),
            local_config=LocalConfig(),
            selected_target=None,
            expected_error_fragment="Named target 'dev' must explicitly set schema",
        ),
        ValidateNamedTargetSchemaTestCase(
            description="local override leaves merged named target schema blank",
            adapter=SnowflakeAdapter(),
            project_config=ProjectConfig(
                name="test", adapter="snowflake", targets={"ci": TargetConfig()}
            ),
            local_config=LocalConfig(targets={"ci": LocalTargetConfig(schema="   ")}),
            selected_target="ci",
            expected_error_fragment="Named target 'ci' must explicitly set schema",
        ),
        ValidateNamedTargetSchemaTestCase(
            description="MotherDuck named target omits schema",
            adapter=MotherDuckAdapter(),
            project_config=ProjectConfig(
                name="test",
                adapter="motherduck",
                default_target="dev",
                targets={"dev": TargetConfig()},
            ),
            local_config=LocalConfig(),
            selected_target=None,
            expected_error_fragment="Named target 'dev' must explicitly set schema",
        ),
        ValidateNamedTargetSchemaTestCase(
            description="BigQuery named target omits schema",
            adapter=BigQueryAdapter(),
            project_config=ProjectConfig(
                name="test",
                adapter="bigquery",
                default_target="dev",
                targets={"dev": TargetConfig()},
            ),
            local_config=LocalConfig(),
            selected_target=None,
            expected_error_fragment="Named target 'dev' must explicitly set schema",
        ),
        ValidateNamedTargetSchemaTestCase(
            description="Databricks named target omits schema",
            adapter=DatabricksAdapter(),
            project_config=ProjectConfig(
                name="test",
                adapter="databricks",
                default_target="dev",
                targets={"dev": TargetConfig()},
            ),
            local_config=LocalConfig(),
            selected_target=None,
            expected_error_fragment="Named target 'dev' must explicitly set schema",
        ),
        ValidateNamedTargetSchemaTestCase(
            description="SQL Server named target omits schema",
            adapter=SqlServerAdapter(),
            project_config=ProjectConfig(
                name="test",
                adapter="sqlserver",
                default_target="dev",
                targets={"dev": TargetConfig()},
            ),
            local_config=LocalConfig(),
            selected_target=None,
            expected_error_fragment="Named target 'dev' must explicitly set schema",
        ),
        ValidateNamedTargetSchemaTestCase(
            description="custom adapter named target omits schema conservatively",
            adapter=ConservativeCustomAdapter(),
            project_config=ProjectConfig(
                name="test", adapter="custom", default_target="dev", targets={"dev": TargetConfig()}
            ),
            local_config=LocalConfig(),
            selected_target=None,
            expected_error_fragment="Named target 'dev' must explicitly set schema",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_named_target_without_schema_when_validating_strategy_then_error_is_raised(
    test_case: ValidateNamedTargetSchemaTestCase,
) -> None:
    discovered_inputs: DiscoveredProjectInputs = DiscoveredProjectInputs(
        project_config=test_case.project_config,
        local_config=test_case.local_config,
    )

    with pytest.raises(ValueError, match=str(test_case.expected_error_fragment)):
        validate_named_target_schema_strategy(
            discovered_inputs=discovered_inputs,
            adapter=test_case.adapter,
            selected_target=test_case.selected_target,
        )


@pytest.mark.parametrize(
    "test_case",
    (
        ValidateNamedTargetSchemaTestCase(
            description="literal named target schema is explicit",
            adapter=BigQueryAdapter(),
            project_config=ProjectConfig(
                name="test",
                adapter="bigquery",
                default_target="dev",
                targets={"dev": TargetConfig(schema="analytics")},
            ),
            local_config=LocalConfig(),
            selected_target=None,
            expected_error_fragment=None,
        ),
        ValidateNamedTargetSchemaTestCase(
            description="preserve named target schema is explicit",
            adapter=DatabricksAdapter(),
            project_config=ProjectConfig(
                name="test",
                adapter="databricks",
                default_target="dev",
                targets={"dev": TargetConfig(schema="preserve")},
            ),
            local_config=LocalConfig(),
            selected_target=None,
            expected_error_fragment=None,
        ),
        ValidateNamedTargetSchemaTestCase(
            description="named local DuckDB retains implicit main capability",
            adapter=DuckDbAdapter(),
            project_config=ProjectConfig(
                name="test", adapter="duckdb", default_target="dev", targets={"dev": TargetConfig()}
            ),
            local_config=LocalConfig(),
            selected_target=None,
            expected_error_fragment=None,
        ),
        ValidateNamedTargetSchemaTestCase(
            description="targetless shared warehouse remains under resource write policy",
            adapter=SqlServerAdapter(),
            project_config=ProjectConfig(name="test", adapter="sqlserver"),
            local_config=LocalConfig(),
            selected_target=None,
            expected_error_fragment=None,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_explicit_or_exempt_target_when_validating_schema_strategy_then_it_passes(
    test_case: ValidateNamedTargetSchemaTestCase,
) -> None:
    discovered_inputs: DiscoveredProjectInputs = DiscoveredProjectInputs(
        project_config=test_case.project_config,
        local_config=test_case.local_config,
    )

    validate_named_target_schema_strategy(
        discovered_inputs=discovered_inputs,
        adapter=test_case.adapter,
        selected_target=test_case.selected_target,
    )
    assert test_case.expected_error_fragment is None
