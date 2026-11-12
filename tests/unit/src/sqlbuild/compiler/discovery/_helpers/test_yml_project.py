from __future__ import annotations

import re
from decimal import Decimal
from pathlib import Path

import pytest

from sqlbuild._native import (
    AuthoredTimeTravelRetention,
    CollectionRendering,
    ExecutionLimitsConfig,
    LocalConfig,
    MissingMigrationOriginPolicy,
    ProjectConfig,
    ProjectConfigError,
    RetentionDecreasePolicy,
    TargetConfig,
)
from sqlbuild.runtime.event_exporting.types import LifecycleEventKind
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    CheckDeclaredColumnsConfigErrorTestCase,
    CheckDeclaredColumnsConfigTestCase,
    DeepProjectConnectionTestCase,
    DiffConfigTestCase,
    EffectiveBatchDefaultTestCase,
    FutureCursorConfigErrorTestCase,
    FutureCursorConfigTestCase,
    LoadLocalConfigErrorTestCase,
    LoadLocalConfigTestCase,
    LoadNamedConnectionsTestCase,
    LoadProjectConfigErrorTestCase,
    LoadProjectConfigHelpTestCase,
    LoadProjectConfigTestCase,
    LoadProjectConstantsConfigErrorTestCase,
    LoadProjectConstantsConfigTestCase,
    LoadProjectCostConfigErrorTestCase,
    LoadProjectCostConfigTestCase,
    LoadRetentionConfigErrorTestCase,
    LoadRetentionConfigTestCase,
    LoadTargetRetentionPoliciesErrorTestCase,
    LoadTargetRetentionPoliciesTestCase,
    MicrobatchLimitConfigErrorTestCase,
    MicrobatchLimitConfigTestCase,
    MissingMigrationOriginConfigTestCase,
    OversizedDurationTestCase,
    StartCursorConfigTestCase,
    TargetExecutionLimitsConfigTestCase,
    TooDeepProjectConnectionTestCase,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers.helpers import (
    load_local_config,
    load_project_config,
    write_project_config_test_files,
)


@pytest.mark.parametrize(
    "test_case",
    [DiffConfigTestCase(description="positive fixed TTL", expected_query_artifact_ttl="6h")],
    ids=lambda case: case.description,
)
def test_given_query_artifact_ttl_when_loading_project_then_diff_config_is_typed(
    test_case: DiffConfigTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "demo"\nadapter = "duckdb"\n[diff]\n'
        f'query_artifact_ttl = "{test_case.expected_query_artifact_ttl}"\n',
        encoding="utf-8",
    )

    config: ProjectConfig = load_project_config(project_dir=tmp_path)

    assert config.diff.query_artifact_ttl == test_case.expected_query_artifact_ttl


@pytest.mark.parametrize(
    "test_case",
    [
        DiffConfigTestCase(
            description="zero TTL",
            expected_query_artifact_ttl="0h",
            expected_error_fragment="query_artifact_ttl must be a positive",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_query_artifact_ttl_when_loading_project_then_error_is_actionable(
    test_case: DiffConfigTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "demo"\nadapter = "duckdb"\n[diff]\n'
        f'query_artifact_ttl = "{test_case.expected_query_artifact_ttl}"\n',
        encoding="utf-8",
    )

    with pytest.raises(ProjectConfigError, match=test_case.expected_error_fragment or ""):
        _ = load_project_config(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        LoadProjectConfigErrorTestCase(
            description="audit lifecycle sink filter",
            project_file_contents=(
                'name = "demo"\nadapter = "duckdb"\n[sinks.lifecycle]\nevent_kinds = ["audit"]\n'
            ),
            expected_error_fragment="audit",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_audit_lifecycle_sink_filter_when_loading_toml_then_kind_is_accepted(
    tmp_path: Path, test_case: LoadProjectConfigErrorTestCase
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        test_case.project_file_contents, encoding="utf-8"
    )

    config: ProjectConfig = load_project_config(project_dir=tmp_path)

    assert config.sinks.lifecycle.defaults.event_kinds == frozenset(
        {test_case.expected_error_fragment}
    )
    assert test_case.expected_error_fragment == LifecycleEventKind.AUDIT.value


@pytest.mark.parametrize(
    "test_case",
    [
        LoadProjectCostConfigTestCase(
            description="omitted cost config uses flagged default rate",
            project_file_contents='name = "demo"\nadapter = "duckdb"\n',
            expected_usd_per_credit=Decimal("3.00"),
            expected_is_default=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_no_cost_config_when_loading_project_then_default_rate_is_flagged(
    test_case: LoadProjectCostConfigTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        test_case.project_file_contents, encoding="utf-8"
    )

    config: ProjectConfig = load_project_config(project_dir=tmp_path)

    assert config.cost.usd_per_credit == test_case.expected_usd_per_credit
    assert config.cost.usd_per_credit_is_default is test_case.expected_is_default


@pytest.mark.parametrize(
    "test_case",
    [
        FutureCursorConfigTestCase(
            description="future cap policy is typed",
            future_toml='max_distance = "7d"\naction = "cap"',
            expected_max_distance="7d",
            expected_action="cap",
        ),
        FutureCursorConfigTestCase(
            description="zero-day future cap policy is typed",
            future_toml='max_distance = "0d"\naction = "cap"',
            expected_max_distance="0d",
            expected_action="cap",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_future_cursor_config_when_loading_project_then_policy_is_typed(
    test_case: FutureCursorConfigTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "demo"\nadapter = "duckdb"\n[cursors.future]\n{test_case.future_toml}\n',
        encoding="utf-8",
    )

    config: ProjectConfig = load_project_config(project_dir=tmp_path)

    assert config.cursors.future.max_distance == test_case.expected_max_distance
    assert config.cursors.future.action == test_case.expected_action


@pytest.mark.parametrize(
    "test_case",
    [
        StartCursorConfigTestCase(
            description="zero-ahead cap policy is typed",
            start_toml='max_ahead = "0d"\naction = "cap"',
            expected_max_ahead="0d",
            expected_action="cap",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_start_cursor_config_when_loading_project_then_policy_is_typed(
    test_case: StartCursorConfigTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "demo"\nadapter = "duckdb"\n[cursors.start]\n{test_case.start_toml}\n',
        encoding="utf-8",
    )

    config: ProjectConfig = load_project_config(project_dir=tmp_path)

    assert config.cursors.start.max_ahead == test_case.expected_max_ahead
    assert config.cursors.start.action == test_case.expected_action


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchLimitConfigTestCase(
            description="configured warn limit is typed",
            limits_toml='max_batches = 24\naction = "warn"',
            expected_max_batches=24,
            expected_action="warn",
        ),
        MicrobatchLimitConfigTestCase(
            description="absent max batches disables limit",
            limits_toml='action = "error"',
            expected_max_batches=None,
            expected_action="error",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_microbatch_limit_config_when_loading_project_then_policy_is_typed(
    test_case: MicrobatchLimitConfigTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "demo"\nadapter = "duckdb"\n[microbatches.limits]\n{test_case.limits_toml}\n',
        encoding="utf-8",
    )

    config: ProjectConfig = load_project_config(project_dir=tmp_path)

    assert config.microbatches.limits.max_batches == test_case.expected_max_batches
    assert config.microbatches.limits.action == test_case.expected_action


@pytest.mark.parametrize(
    "test_case",
    [
        EffectiveBatchDefaultTestCase(
            description="effective token is preserved",
            project_file_contents=(
                'name = "demo"\nadapter = "duckdb"\n[defaults]\nbatch_size = "effective"\n'
            ),
            expected_batch_size="effective",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_effective_batch_size_default_when_loading_project_then_token_is_preserved(
    test_case: EffectiveBatchDefaultTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        test_case.project_file_contents, encoding="utf-8"
    )

    config: ProjectConfig = load_project_config(project_dir=tmp_path)

    assert config.defaults.batch_size == test_case.expected_batch_size


@pytest.mark.parametrize(
    "test_case",
    [
        MicrobatchLimitConfigErrorTestCase(
            description="zero max batches is rejected",
            limits_toml="max_batches = 0",
            expected_error_fragment="must be a positive integer",
        ),
        MicrobatchLimitConfigErrorTestCase(
            description="boolean max batches is rejected",
            limits_toml="max_batches = true",
            expected_error_fragment="must be a positive integer",
        ),
        MicrobatchLimitConfigErrorTestCase(
            description="unsupported limit action is rejected",
            limits_toml='max_batches = 2\naction = "cap"',
            expected_error_fragment="action must be one of: error, warn",
        ),
        MicrobatchLimitConfigErrorTestCase(
            description="model cap action is rejected as project safety policy",
            limits_toml='max_batches = 2\naction = "cap_from_end"',
            expected_error_fragment="action must be one of: error, warn",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_microbatch_limit_when_loading_project_then_error_is_raised(
    test_case: MicrobatchLimitConfigErrorTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "demo"\nadapter = "duckdb"\n[microbatches.limits]\n{test_case.limits_toml}\n',
        encoding="utf-8",
    )

    with pytest.raises(ProjectConfigError, match=test_case.expected_error_fragment):
        load_project_config(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        FutureCursorConfigErrorTestCase(
            description="invalid duration",
            future_toml='max_distance = "tomorrow"',
            expected_error_fragment="max_distance must be a duration",
        ),
        FutureCursorConfigErrorTestCase(
            description="invalid action",
            future_toml='max_distance = "7d"\naction = "warn"',
            expected_error_fragment="action must be one of: cap, error",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_future_cursor_config_when_loading_project_then_config_error_is_raised(
    test_case: FutureCursorConfigErrorTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "demo"\nadapter = "duckdb"\n[cursors.future]\n{test_case.future_toml}\n',
        encoding="utf-8",
    )

    with pytest.raises(ProjectConfigError, match=test_case.expected_error_fragment):
        load_project_config(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        LoadProjectCostConfigTestCase(
            description="numeric cost config preserves exact configured rate",
            project_file_contents=(
                'name = "demo"\nadapter = "snowflake"\n[cost]\nusd_per_credit = 3.25\n'
            ),
            expected_usd_per_credit=Decimal("3.25"),
            expected_is_default=False,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_numeric_cost_config_when_loading_project_then_exact_rate_is_configured(
    test_case: LoadProjectCostConfigTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        test_case.project_file_contents,
        encoding="utf-8",
    )

    config: ProjectConfig = load_project_config(project_dir=tmp_path)

    assert config.cost.usd_per_credit == test_case.expected_usd_per_credit
    assert config.cost.usd_per_credit_is_default is test_case.expected_is_default


@pytest.mark.parametrize(
    "test_case",
    [
        LoadProjectCostConfigErrorTestCase(
            description="boolean cost rate is rejected",
            value="true",
            expected_error_fragment="cost.usd_per_credit",
        ),
        LoadProjectCostConfigErrorTestCase(
            description="string cost rate is rejected",
            value='"3.00"',
            expected_error_fragment="cost.usd_per_credit",
        ),
        LoadProjectCostConfigErrorTestCase(
            description="zero cost rate is rejected",
            value="0",
            expected_error_fragment="cost.usd_per_credit",
        ),
        LoadProjectCostConfigErrorTestCase(
            description="negative cost rate is rejected",
            value="-1",
            expected_error_fragment="cost.usd_per_credit",
        ),
        LoadProjectCostConfigErrorTestCase(
            description="infinite cost rate is rejected",
            value="inf",
            expected_error_fragment="cost.usd_per_credit",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_cost_rate_when_loading_project_then_config_error_is_raised(
    test_case: LoadProjectCostConfigErrorTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "demo"\nadapter = "snowflake"\n[cost]\nusd_per_credit = {test_case.value}\n',
        encoding="utf-8",
    )

    with pytest.raises(ProjectConfigError, match=test_case.expected_error_fragment):
        load_project_config(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        LoadProjectConstantsConfigTestCase(
            description="omitted constants config uses value-list rendering",
            constants_toml="",
            expected_collection_rendering=CollectionRendering.VALUE_LIST,
        ),
        LoadProjectConstantsConfigTestCase(
            description="array collection rendering is loaded",
            constants_toml='[constants]\ncollection_rendering = "array"',
            expected_collection_rendering=CollectionRendering.ARRAY,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_constants_config_when_loading_project_then_collection_rendering_is_typed(
    test_case: LoadProjectConstantsConfigTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "demo"\nadapter = "duckdb"\n{test_case.constants_toml}\n',
        encoding="utf-8",
    )

    config: ProjectConfig = load_project_config(project_dir=tmp_path)

    assert config.constants.collection_rendering is test_case.expected_collection_rendering


@pytest.mark.parametrize(
    "test_case",
    [
        LoadProjectConstantsConfigErrorTestCase(
            description="non-string collection rendering is rejected",
            constants_toml="[constants]\ncollection_rendering = true",
            expected_error_fragment="constants.collection_rendering must be a string",
        ),
        LoadProjectConstantsConfigErrorTestCase(
            description="unknown collection rendering is rejected",
            constants_toml='[constants]\ncollection_rendering = "values"',
            expected_error_fragment="constants.collection_rendering must be one of: value_list, array",
        ),
        LoadProjectConstantsConfigErrorTestCase(
            description="unknown constants key is rejected",
            constants_toml='[constants]\nrendering = "array"',
            expected_error_fragment=r"constants contains unknown key\(s\): rendering",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_constants_config_when_loading_project_then_config_error_is_raised(
    test_case: LoadProjectConstantsConfigErrorTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "demo"\nadapter = "duckdb"\n{test_case.constants_toml}\n',
        encoding="utf-8",
    )

    with pytest.raises(ProjectConfigError, match=test_case.expected_error_fragment):
        load_project_config(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        LoadRetentionConfigTestCase(
            description="duration and disabled retention policies",
            project_file_contents="\n".join(
                (
                    'name = "demo"',
                    'adapter = "snowflake"',
                    "[materialization_defaults.table]",
                    'time_travel_retention = "14d"',
                    "[materialization_defaults.incremental]",
                    'time_travel_retention = "disabled"',
                    "[targets.prod]",
                    'time_travel_retention = "0d"',
                )
            ),
            expected_table_days=14,
            expected_incremental_unmanaged=True,
            expected_target_days=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_retention_config_when_loading_project_then_policies_are_typed(
    test_case: LoadRetentionConfigTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        test_case.project_file_contents,
        encoding="utf-8",
    )

    config: ProjectConfig = load_project_config(project_dir=tmp_path)

    assert config.materialization_defaults.table.time_travel_retention is not None
    assert (
        config.materialization_defaults.table.time_travel_retention.desired_days
        == test_case.expected_table_days
    )
    assert config.materialization_defaults.incremental.time_travel_retention is not None
    assert (
        config.materialization_defaults.incremental.time_travel_retention.unmanaged
        is test_case.expected_incremental_unmanaged
    )
    assert config.targets["prod"].time_travel_retention is not None
    assert (
        config.targets["prod"].time_travel_retention.desired_days == test_case.expected_target_days
    )


@pytest.mark.parametrize(
    "test_case",
    [
        LoadTargetRetentionPoliciesTestCase(
            description="string retention is a target-wide default and decreases are denied",
            target_lines=('time_travel_retention = "30d"',),
            expected_default=AuthoredTimeTravelRetention(desired_days=30),
            expected_by_materialization={},
            expected_decrease_policy=RetentionDecreasePolicy.DENY,
        ),
        LoadTargetRetentionPoliciesTestCase(
            description="retention table sets per-materialization values",
            target_lines=(
                'time_travel_retention = { table = "90d", snapshot = "365d" }',
                'time_travel_retention_decrease = "require_confirmation"',
            ),
            expected_default=None,
            expected_by_materialization={
                "table": AuthoredTimeTravelRetention(desired_days=90),
                "snapshot": AuthoredTimeTravelRetention(desired_days=365),
            },
            expected_decrease_policy=RetentionDecreasePolicy.REQUIRE_CONFIRMATION,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_target_retention_policies_when_loading_project_then_they_are_typed(
    test_case: LoadTargetRetentionPoliciesTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        "\n".join(
            ('name = "demo"', 'adapter = "snowflake"', "[targets.prod]", *test_case.target_lines)
        ),
        encoding="utf-8",
    )

    target: TargetConfig = load_project_config(project_dir=tmp_path).targets["prod"]

    assert target.time_travel_retention == test_case.expected_default
    assert target.time_travel_retention_by_materialization == test_case.expected_by_materialization
    assert target.time_travel_retention_decrease is test_case.expected_decrease_policy


@pytest.mark.parametrize(
    "test_case",
    [
        MissingMigrationOriginConfigTestCase(
            description="missing migration origins are allowed by default",
            target_lines=(),
            expected_policy=MissingMigrationOriginPolicy.ALLOW,
        ),
        MissingMigrationOriginConfigTestCase(
            description="deny is typed",
            target_lines=('missing_migration_origin = "deny"',),
            expected_policy=MissingMigrationOriginPolicy.DENY,
        ),
        MissingMigrationOriginConfigTestCase(
            description="require_confirmation is typed",
            target_lines=('missing_migration_origin = "require_confirmation"',),
            expected_policy=MissingMigrationOriginPolicy.REQUIRE_CONFIRMATION,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_missing_migration_origin_policy_when_loading_project_then_it_is_typed(
    test_case: MissingMigrationOriginConfigTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        "\n".join(
            ('name = "demo"', 'adapter = "duckdb"', "[targets.prod]", *test_case.target_lines)
        ),
        encoding="utf-8",
    )

    target: TargetConfig = load_project_config(project_dir=tmp_path).targets["prod"]

    assert target.missing_migration_origin is test_case.expected_policy


@pytest.mark.parametrize(
    "test_case",
    [
        LoadTargetRetentionPoliciesErrorTestCase(
            description="retention table rejects non-table materializations",
            target_lines=('time_travel_retention = { view = "7d" }',),
            expected_error_fragment="view",
        ),
        LoadTargetRetentionPoliciesErrorTestCase(
            description="retention table values must be whole days",
            target_lines=('time_travel_retention = { table = "12h" }',),
            expected_error_fragment="targets.prod.time_travel_retention.table",
        ),
        LoadTargetRetentionPoliciesErrorTestCase(
            description="decrease policy must be a known value",
            target_lines=('time_travel_retention_decrease = "sometimes"',),
            expected_error_fragment="'deny', 'require_confirmation', or 'allow'",
        ),
        LoadTargetRetentionPoliciesErrorTestCase(
            description="missing migration origin policy must be a known value",
            target_lines=('missing_migration_origin = "sometimes"',),
            expected_error_fragment="targets.prod.missing_migration_origin must be",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_target_retention_policies_when_loading_project_then_config_error_is_raised(
    test_case: LoadTargetRetentionPoliciesErrorTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        "\n".join(
            ('name = "demo"', 'adapter = "snowflake"', "[targets.prod]", *test_case.target_lines)
        ),
        encoding="utf-8",
    )

    with pytest.raises(ProjectConfigError, match=test_case.expected_error_fragment):
        load_project_config(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        LoadRetentionConfigErrorTestCase(
            description="hour retention value",
            project_file_contents="\n".join(
                (
                    'name = "demo"',
                    'adapter = "snowflake"',
                    "[materialization_defaults.table]",
                    'time_travel_retention = "12h"',
                )
            ),
            expected_error_fragment="whole-day string",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_non_day_retention_when_loading_project_then_config_error_is_raised(
    test_case: LoadRetentionConfigErrorTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        test_case.project_file_contents,
        encoding="utf-8",
    )

    with pytest.raises(ProjectConfigError, match=test_case.expected_error_fragment):
        load_project_config(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        LoadProjectConfigTestCase(
            description="defaults environment mode to direct when omitted",
            project_file_contents="""
name = "demo"
adapter = "duckdb"
default_target = "dev"

[targets.dev]
schema = "dev"
""".strip(),
            expected_name="demo",
            expected_adapter="duckdb",
            expected_default_target="dev",
            expected_connection={},
            expected_sql_analysis=True,
            expected_max_concurrency=1,
            expected_materialized=None,
            expected_row_diff_exclude_columns=(),
            expected_row_diff_tolerances={},
            expected_contract=None,
            expected_path_defaults={},
            expected_vars={},
            expected_targets={
                "dev": {
                    "connection": {},
                    "vars": {},
                    "database": None,
                    "schema": "dev",
                    "loader_schema": None,
                    "defer_sources_to": None,
                    "defer_clone_from": None,
                    "compile_cache": None,
                    "allow_as_clone_origin": False,
                    "allow_as_clone_destination": False,
                }
            },
            expected_janitor_enabled=False,
            expected_retention_days=None,
            expected_janitor_delete_tracked_only=True,
            expected_janitor_exclude_patterns=(),
        ),
        LoadProjectConfigTestCase(
            description="loads expected fields from project config",
            project_file_contents="""
name = "demo"
adapter = "duckdb"
default_target = "dev"

[connection]
path = "data.db"

[settings]
sql_analysis = false
query_change_tracking = true
concurrency = 8
auto_load_sources = false

[scopes]
enforce_placement = false

[references]
enforce_explicit = false

[defaults]
materialized = "table"
seed_database = "seed_db"
seed_schema = "seed_schema"
function_database = "function_db"
function_schema = "function_schema"
row_diff_exclude_columns = ["loaded_at"]
contract = "enforced"

[defaults.row_diff_tolerances.by_type.float]
relative = 0.0001
absolute = 0.000001

[defaults.row_diff_tolerances.by_column.revenue]
absolute = 0.01

[path_defaults.staging]
schema = "staging"

[vars]
user = "kevin"

[targets.dev]
schema = "dev_${user}"
loader_schema = "raw_${user}"
defer_sources_to = "prod"
defer_clone_from = "prod"
compile_cache = false

[targets.dev.connection]
warehouse = "dev_wh"

[targets.dev.vars]
schema_prefix = "dev"

[targets.dev.clone]
allow_as_clone_origin = true
allow_as_clone_destination = true

[janitor]
enabled = true
retention_days = 21
archive_retention_days = 3
delete_tracked_only = false
exclude_patterns = ["partition_*"]
direct_state_history_versions = 5

[snapshots]
current_state_full_refresh = "require_confirmation"
historical_full_refresh = "allow"
schema_change = "deny"
wildcard_check_schema_change = "append_new_columns"

[scenario.local_type_overrides.snowflake]
"NUMBER(*,0)" = "BIGINT"
OBJECT = "JSON"

[scenario.local_type_overrides.bigquery]
"BIGNUMERIC(*,*)" = "DECIMAL({1}, {2})"

[scenario.snapshot_limits]
max_rows_per_relation = 100
max_total_rows = 200
max_bytes_per_relation = 300
max_total_bytes = 400

[dbt]
project_dir = "../dbt"
profiles_dir = "../profiles"
target = "prod"
target_path = "target/dbt"

[dbt.vars]
country = "US"
limit = 100
enabled = true
""".strip(),
            expected_name="demo",
            expected_adapter="duckdb",
            expected_default_target="dev",
            expected_connection={"path": "data.db"},
            expected_sql_analysis=False,
            expected_max_concurrency=8,
            expected_auto_load_sources=False,
            expected_enforce_placement=False,
            expected_enforce_explicit_references=False,
            expected_materialized="table",
            expected_row_diff_exclude_columns=("loaded_at",),
            expected_row_diff_tolerances={
                "by_type": {
                    "float": {"relative": 0.0001, "absolute": 0.000001},
                },
                "by_column": {
                    "revenue": {"absolute": 0.01},
                },
            },
            expected_contract="enforced",
            expected_seed_database="seed_db",
            expected_seed_schema="seed_schema",
            expected_function_database="function_db",
            expected_function_schema="function_schema",
            expected_path_defaults={"staging": {"schema": "staging"}},
            expected_vars={"user": "kevin"},
            expected_targets={
                "dev": {
                    "connection": {"warehouse": "dev_wh"},
                    "vars": {"schema_prefix": "dev"},
                    "database": None,
                    "schema": "dev_${user}",
                    "loader_schema": "raw_${user}",
                    "defer_sources_to": "prod",
                    "defer_clone_from": "prod",
                    "compile_cache": False,
                    "allow_as_clone_origin": True,
                    "allow_as_clone_destination": True,
                }
            },
            expected_janitor_enabled=True,
            expected_retention_days=21,
            expected_janitor_archive_retention_days=3,
            expected_janitor_delete_tracked_only=False,
            expected_janitor_exclude_patterns=("partition_*",),
            expected_janitor_direct_state_history_versions=5,
            expected_current_state_full_refresh="require_confirmation",
            expected_historical_full_refresh="allow",
            expected_snapshot_schema_change="deny",
            expected_wildcard_check_schema_change="append_new_columns",
            expected_scenario_local_type_overrides={
                "snowflake": {
                    "NUMBER(*,0)": "BIGINT",
                    "OBJECT": "JSON",
                },
                "bigquery": {"BIGNUMERIC(*,*)": "DECIMAL({1}, {2})"},
            },
            expected_snapshot_limits={
                "max_rows_per_relation": 100,
                "max_total_rows": 200,
                "max_bytes_per_relation": 300,
                "max_total_bytes": 400,
            },
            expected_dbt_project_dir="../dbt",
            expected_dbt_profiles_dir="../profiles",
            expected_dbt_target="prod",
            expected_dbt_target_path="target/dbt",
            expected_dbt_vars={"country": "US", "limit": 100, "enabled": True},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_config_file_when_loading_project_config_then_it_returns_expected_fields(
    test_case: LoadProjectConfigTestCase,
    tmp_path: Path,
) -> None:
    write_project_config_test_files(tmp_path=tmp_path, test_case=test_case)

    config: object = load_project_config(project_dir=tmp_path)

    assert config.name == test_case.expected_name
    assert config.adapter == test_case.expected_adapter
    assert config.default_target == test_case.expected_default_target
    assert config.connection == test_case.expected_connection
    assert config.settings.sql_analysis is test_case.expected_sql_analysis
    assert config.settings.concurrency == test_case.expected_max_concurrency
    assert config.settings.auto_load_sources is test_case.expected_auto_load_sources
    assert config.scopes.enforce_placement is test_case.expected_enforce_placement
    assert config.references.enforce_explicit is test_case.expected_enforce_explicit_references
    assert config.defaults.materialized == test_case.expected_materialized
    assert config.defaults.row_diff_exclude_columns == test_case.expected_row_diff_exclude_columns
    assert config.defaults.row_diff_tolerances == test_case.expected_row_diff_tolerances
    assert config.defaults.contract == test_case.expected_contract
    assert config.defaults.seed_database == test_case.expected_seed_database
    assert config.defaults.seed_schema == test_case.expected_seed_schema
    assert config.defaults.function_database == test_case.expected_function_database
    assert config.defaults.function_schema == test_case.expected_function_schema
    assert config.path_defaults == test_case.expected_path_defaults
    assert config.vars == test_case.expected_vars
    assert config.janitor.enabled is test_case.expected_janitor_enabled
    assert config.janitor.retention_days == test_case.expected_retention_days
    assert (
        config.janitor.archive_retention_days == test_case.expected_janitor_archive_retention_days
    )
    assert config.janitor.delete_tracked_only is test_case.expected_janitor_delete_tracked_only
    assert config.janitor.exclude_patterns == test_case.expected_janitor_exclude_patterns
    assert (
        config.janitor.direct_state_history_versions
        == test_case.expected_janitor_direct_state_history_versions
    )
    assert (
        config.snapshots.current_state_full_refresh == test_case.expected_current_state_full_refresh
    )
    assert config.snapshots.historical_full_refresh == test_case.expected_historical_full_refresh
    assert config.snapshots.schema_change == test_case.expected_snapshot_schema_change
    assert (
        config.snapshots.wildcard_check_schema_change
        == test_case.expected_wildcard_check_schema_change
    )
    assert config.scenario.local_type_overrides == test_case.expected_scenario_local_type_overrides
    assert {
        "max_rows_per_relation": config.scenario.snapshot_limits.max_rows_per_relation,
        "max_total_rows": config.scenario.snapshot_limits.max_total_rows,
        "max_bytes_per_relation": config.scenario.snapshot_limits.max_bytes_per_relation,
        "max_total_bytes": config.scenario.snapshot_limits.max_total_bytes,
    } == test_case.expected_snapshot_limits
    assert config.dbt.project_dir == test_case.expected_dbt_project_dir
    assert config.dbt.profiles_dir == test_case.expected_dbt_profiles_dir
    assert config.dbt.target == test_case.expected_dbt_target
    assert config.dbt.target_path == test_case.expected_dbt_target_path
    assert config.dbt.vars == test_case.expected_dbt_vars


@pytest.mark.parametrize(
    "test_case",
    [
        LoadLocalConfigTestCase(
            description="defaults cleanly when local file is missing",
            repo_files={},
            expected_target=None,
            expected_adapter=None,
            expected_connection={},
            expected_sql_analysis=True,
            expected_sql_validation=True,
            expected_max_concurrency=1,
            expected_setting_overrides=frozenset(),
            expected_vars={},
        ),
        LoadLocalConfigTestCase(
            description="loads environment connection settings and vars from local config",
            repo_files={
                "sqlbuild_local.toml": """
target = "dev"
adapter = "snowflake"

[connection]
database = "local.duckdb"

[settings]
sql_analysis = false
sql_validation = false
concurrency = 4
auto_load_sources = false

[vars]
user = "kevin"

[dbt]
target = "pat"

[dbt.vars]
shared = "local"
threads = 2

[scenario.local_type_overrides.snowflake]
"NUMBER(*,0)" = "BIGINT"
OBJECT = "JSON"

[scenario.snapshot_limits]
max_rows_per_relation = 12
max_total_rows = 34
max_bytes_per_relation = 56
max_total_bytes = 78
""".strip()
            },
            expected_target="dev",
            expected_adapter="snowflake",
            expected_connection={"database": "local.duckdb"},
            expected_sql_analysis=False,
            expected_sql_validation=False,
            expected_max_concurrency=4,
            expected_auto_load_sources=False,
            expected_setting_overrides=frozenset(
                {
                    "sql_analysis",
                    "concurrency",
                    "auto_load_sources",
                }
            ),
            expected_vars={"user": "kevin"},
            expected_dbt_target="pat",
            expected_dbt_vars={"shared": "local", "threads": 2},
            expected_scenario_local_type_overrides={
                "snowflake": {
                    "NUMBER(*,0)": "BIGINT",
                    "OBJECT": "JSON",
                }
            },
            expected_snapshot_limits={
                "max_rows_per_relation": 12,
                "max_total_rows": 34,
                "max_bytes_per_relation": 56,
                "max_total_bytes": 78,
            },
        ),
        LoadLocalConfigTestCase(
            description="loads legacy local max concurrency as canonical concurrency override",
            repo_files={
                "sqlbuild_local.toml": """
[settings]
max_concurrency = 4
""".strip()
            },
            expected_target=None,
            expected_adapter=None,
            expected_connection={},
            expected_sql_analysis=True,
            expected_sql_validation=True,
            expected_max_concurrency=4,
            expected_setting_overrides=frozenset({"concurrency"}),
            expected_vars={},
        ),
        LoadLocalConfigTestCase(
            description="does not expose unsupported project level overrides",
            repo_files={
                "sqlbuild_local.toml": """
default_target = "prod"

[defaults]
materialized = "table"

[janitor]
enabled = true

[connection]
database = "local.duckdb"
""".strip()
            },
            expected_target=None,
            expected_adapter=None,
            expected_connection={"database": "local.duckdb"},
            expected_sql_analysis=True,
            expected_sql_validation=True,
            expected_max_concurrency=1,
            expected_setting_overrides=frozenset(),
            expected_vars={},
            expected_missing_attributes=(
                "default_target",
                "defaults",
                "janitor",
            ),
        ),
        LoadLocalConfigTestCase(
            description="loads local target overrides",
            repo_files={
                "sqlbuild_local.toml": """
target = "dev"

[targets.dev]
database = "local_db"
schema = "local_schema"
loader_schema = "local_raw"
defer_sources_to = "prod"
defer_clone_from = "prod"
compile_cache = false

[targets.dev.connection]
warehouse = "local_wh"

[targets.dev.vars]
user = "local_user"

[targets.dev.clone]
allow_as_clone_origin = true
allow_as_clone_destination = false
""".strip()
            },
            expected_target="dev",
            expected_adapter=None,
            expected_connection={},
            expected_sql_analysis=True,
            expected_sql_validation=True,
            expected_max_concurrency=1,
            expected_setting_overrides=frozenset(),
            expected_vars={},
            expected_targets={
                "dev": {
                    "connection": {"warehouse": "local_wh"},
                    "vars": {"user": "local_user"},
                    "database": "local_db",
                    "schema": "local_schema",
                    "loader_schema": "local_raw",
                    "defer_sources_to": "prod",
                    "defer_clone_from": "prod",
                    "compile_cache": False,
                    "allow_as_clone_origin": True,
                    "allow_as_clone_destination": False,
                }
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_local_config_state_when_loading_local_config_then_it_returns_expected_fields(
    test_case: LoadLocalConfigTestCase,
    tmp_path: Path,
) -> None:
    relative_path: str
    contents: str
    for relative_path, contents in test_case.repo_files.items():
        file_path: Path = tmp_path / relative_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(contents, encoding="utf-8")

    config: object = load_local_config(project_dir=tmp_path)

    assert config.target == test_case.expected_target
    assert config.adapter == test_case.expected_adapter
    assert config.connection == test_case.expected_connection
    assert config.settings.sql_analysis is test_case.expected_sql_analysis
    assert config.settings.sql_validation is test_case.expected_sql_validation
    assert config.settings.concurrency == test_case.expected_max_concurrency
    assert config.settings.auto_load_sources is test_case.expected_auto_load_sources
    assert config.setting_overrides == test_case.expected_setting_overrides
    assert config.vars == test_case.expected_vars
    assert config.dbt.target == test_case.expected_dbt_target
    assert config.dbt.vars == test_case.expected_dbt_vars
    assert config.scenario.local_type_overrides == test_case.expected_scenario_local_type_overrides
    assert {
        "max_rows_per_relation": config.scenario.snapshot_limits.max_rows_per_relation,
        "max_total_rows": config.scenario.snapshot_limits.max_total_rows,
        "max_bytes_per_relation": config.scenario.snapshot_limits.max_bytes_per_relation,
        "max_total_bytes": config.scenario.snapshot_limits.max_total_bytes,
    } == test_case.expected_snapshot_limits
    attribute_name: str
    for attribute_name in test_case.expected_missing_attributes:
        assert not hasattr(config, attribute_name)


@pytest.mark.parametrize(
    "test_case",
    [
        TargetExecutionLimitsConfigTestCase(
            description="project target execution limits are typed",
            contents="""
name = "shop"
adapter = "duckdb"

[targets.dev.execution_limits]
max_models = 125
max_duration = "45m"
remediation = "Review the expanded selection before continuing."
""".strip(),
            expected_max_models=125,
            expected_max_duration="45m",
            expected_max_duration_seconds=2_700,
            expected_remediation="Review the expanded selection before continuing.",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_target_execution_limits_when_loading_then_limits_are_typed(
    test_case: TargetExecutionLimitsConfigTestCase,
    tmp_path: Path,
) -> None:
    config_path: Path = tmp_path / "sqlbuild_project.toml"
    config_path.write_text(test_case.contents, encoding="utf-8")

    config: ProjectConfig = load_project_config(project_dir=tmp_path)

    limits: ExecutionLimitsConfig = config.targets["dev"].execution_limits
    assert limits.max_models == test_case.expected_max_models
    assert limits.max_duration == test_case.expected_max_duration
    assert limits.max_duration_seconds == test_case.expected_max_duration_seconds
    assert limits.remediation == test_case.expected_remediation


@pytest.mark.parametrize(
    "test_case",
    [
        TargetExecutionLimitsConfigTestCase(
            description="local target execution limits are typed",
            contents="""
[targets.dev.execution_limits]
max_models = 200
max_duration = "2h"
remediation = "Automated tools must request approval before changing this policy."
""".strip(),
            expected_max_models=200,
            expected_max_duration="2h",
            expected_max_duration_seconds=7_200,
            expected_remediation=(
                "Automated tools must request approval before changing this policy."
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_local_target_execution_limits_when_loading_then_limits_are_typed(
    test_case: TargetExecutionLimitsConfigTestCase,
    tmp_path: Path,
) -> None:
    config_path: Path = tmp_path / "sqlbuild_local.toml"
    config_path.write_text(test_case.contents, encoding="utf-8")

    config: LocalConfig = load_local_config(project_dir=tmp_path)

    limits: ExecutionLimitsConfig = config.targets["dev"].execution_limits
    assert limits.max_models == test_case.expected_max_models
    assert limits.max_duration == test_case.expected_max_duration
    assert limits.max_duration_seconds == test_case.expected_max_duration_seconds
    assert limits.remediation == test_case.expected_remediation


@pytest.mark.parametrize(
    "test_case",
    [
        LoadProjectConfigErrorTestCase(
            description="raises when target maximum model count is not positive",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[targets.dev.execution_limits]
max_models = 0
""".strip(),
            expected_error_fragment=("targets.dev.execution_limits.max_models must be >= 1"),
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when target maximum duration has a calendar component",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[targets.dev.execution_limits]
max_duration = "1mo"
""".strip(),
            expected_error_fragment=(
                "targets.dev.execution_limits.max_duration must be a positive fixed duration"
            ),
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when scope placement enforcement is not a boolean",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[scopes]
enforce_placement = "no"
""".strip(),
            expected_error_fragment="Expected 'enforce_placement' to be a boolean when provided",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when explicit reference enforcement is not a boolean",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[references]
enforce_explicit = "no"
""".strip(),
            expected_error_fragment="Expected 'enforce_explicit' to be a boolean when provided",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when the references section has an unknown key",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[references]
enforce = false
""".strip(),
            expected_error_fragment="references",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when settings sql_analysis is not a boolean",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[settings]
sql_analysis = 123
""".strip(),
            expected_error_fragment="Expected 'sql_analysis' to be a boolean when provided",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when canonical and legacy analysis settings conflict",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[settings]
sql_analysis = true
sql_validation = false
""".strip(),
            expected_error_fragment=(
                "settings.sql_analysis conflicts with legacy settings.sql_validation"
            ),
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when settings concurrency is not an integer",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[settings]
concurrency = "nope"
""".strip(),
            expected_error_fragment="Expected 'concurrency' to be an integer when provided",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when settings concurrency is below one",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[settings]
concurrency = 0
""".strip(),
            expected_error_fragment="settings.concurrency must be >= 1",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when microbatch concurrency setting is not a boolean",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[settings]
microbatch_concurrency = "yes"
""".strip(),
            expected_error_fragment=(
                "Expected 'microbatch_concurrency' to be a boolean when provided"
            ),
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when default audit severity is unknown",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[settings]
default_audit_severity = "critical"
""".strip(),
            expected_error_fragment=("settings.default_audit_severity must be one of: warn, error"),
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when project unaccounted policy is unknown",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[settings]
microbatch_unaccounted_partition_policy = "ignore"
""".strip(),
            expected_error_fragment=(
                "settings.microbatch_unaccounted_partition_policy must be one of"
            ),
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when canonical and legacy concurrency settings are both provided",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[settings]
concurrency = 4
max_concurrency = 8
""".strip(),
            expected_error_fragment=(
                "settings cannot define both 'concurrency' and legacy 'max_concurrency'"
            ),
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when defaults batch_size has unsupported type",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[defaults.batch_size]
amount = 1
""".strip(),
            expected_error_fragment="Expected 'batch_size' to be a string or integer when provided",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when defaults contract is unknown",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[defaults]
contract = "strict"
""".strip(),
            expected_error_fragment="Expected 'contract' to be one of",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when snapshot current state full refresh policy is unknown",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[snapshots]
current_state_full_refresh = "force"
""".strip(),
            expected_error_fragment="Expected 'current_state_full_refresh' to be one of",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when snapshot historical full refresh policy is not string",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[snapshots]
historical_full_refresh = true
""".strip(),
            expected_error_fragment="Expected 'historical_full_refresh' to be a string",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when snapshot schema change policy is unknown",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[snapshots]
schema_change = "sync_all_columns"
""".strip(),
            expected_error_fragment="Expected 'schema_change' to be one of",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when path defaults child value is not a mapping",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[path_defaults]
staging = "view"
""".strip(),
            expected_error_fragment="path_defaults",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when project vars contain non string value",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[vars]
user = 123
""".strip(),
            expected_error_fragment="expected string value for 'user'",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when connection is not a mapping",
            project_file_contents="""
name = "demo"
adapter = "duckdb"
connection = "bad"
""".strip(),
            expected_error_fragment="Expected 'connection' to be a mapping when provided",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when environments is not a mapping",
            project_file_contents="""
name = "demo"
adapter = "duckdb"
targets = []
""".strip(),
            expected_error_fragment="targets must be a mapping",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when one environment entry is not a mapping",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[targets]
dev = "here"
""".strip(),
            expected_error_fragment="targets.dev must be a mapping",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when environment clone config is not a mapping",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[targets.dev]
clone = "nope"
""".strip(),
            expected_error_fragment="targets.dev.clone must be a mapping",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when environment vars contain non string value",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[targets.dev.vars]
user = false
""".strip(),
            expected_error_fragment="expected string value for 'user'",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when environment connection is blank",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[targets.dev]
connection = "   "
""".strip(),
            expected_error_fragment="connection must be a non-empty string or mapping",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when named connection value is not a mapping",
            project_file_contents='name = "demo"\nadapter = "duckdb"\nconnections = { dev = "bad" }',
            expected_error_fragment="connections.dev must be a mapping",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when named connection name is blank",
            project_file_contents='name = "demo"\nadapter = "duckdb"\n[connections.""]\ndatabase = "x"',
            expected_error_fragment="connections contains an empty name",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when target contains connections typo",
            project_file_contents='name = "demo"\nadapter = "duckdb"\n[targets.dev]\nconnections = "developer"',
            expected_error_fragment=r"targets.dev contains unknown key\(s\): connections",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when clone contains unknown key",
            project_file_contents='name = "demo"\nadapter = "duckdb"\n[targets.dev.clone]\nconnections = true',
            expected_error_fragment=r"targets.dev.clone contains unknown key\(s\): connections",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when environment clone allow_as_clone_origin is not a boolean",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[targets.dev.clone]
allow_as_clone_origin = 123
""".strip(),
            expected_error_fragment="Expected 'allow_as_clone_origin' to be a boolean when provided",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when defaults tags is a string instead of list",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[defaults]
tags = "nightly"
""".strip(),
            expected_error_fragment="defaults.tags",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when path_defaults tags is a string instead of list",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[path_defaults."models/staging"]
tags = "staging"
""".strip(),
            expected_error_fragment="path_defaults.*tags must be a list",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when path_defaults tags contains non-string entry",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[path_defaults."models/staging"]
tags = [123]
""".strip(),
            expected_error_fragment="path_defaults.*tags.*must be strings",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when path defaults key has leading slash",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[path_defaults."/staging"]
schema = "staging"
""".strip(),
            expected_error_fragment="without a leading slash",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when path defaults key has empty path segment",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[path_defaults."staging//nested"]
schema = "staging"
""".strip(),
            expected_error_fragment="empty path segments",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when path defaults uses redundant models prefix",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[path_defaults."models/staging"]
schema = "staging"
""".strip(),
            expected_error_fragment="uses redundant 'models/' prefix",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when path defaults uses partial segment glob",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[path_defaults."commerce/stag*"]
schema = "staging"
""".strip(),
            expected_error_fragment=r"Use '\*' or '\*\*' as complete path segments",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when tracked-only janitor is enabled without query tracking",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[settings]
query_change_tracking = false

[janitor]
enabled = true
""".strip(),
            expected_error_fragment="delete_tracked_only = true requires query change tracking",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when janitor direct state history versions is negative",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[janitor]
direct_state_history_versions = -1
""".strip(),
            expected_error_fragment="janitor.direct_state_history_versions must be >= 0",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when janitor archive retention days is negative",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[janitor]
archive_retention_days = -1
""".strip(),
            expected_error_fragment="janitor.archive_retention_days must be >= 0",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when old name view retention is not a duration",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[migrations]
old_name_views = 30
""".strip(),
            expected_error_fragment="migrations.old_name_views must be a positive duration",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when migrations contain unknown key",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[migrations]
old_name_view = "7d"
""".strip(),
            expected_error_fragment=r"migrations contains unknown key\(s\): old_name_view",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when project settings contain unknown key",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[settings]
threads = 4
""".strip(),
            expected_error_fragment=r"settings contains unknown key\(s\): threads",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when scenario snapshot limit is not an integer",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[scenario.snapshot_limits]
max_rows_per_relation = "many"
""".strip(),
            expected_error_fragment="Expected 'max_rows_per_relation' to be an integer when provided",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when scenario snapshot limit is negative",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[scenario.snapshot_limits]
max_total_bytes = -1
""".strip(),
            expected_error_fragment="scenario.snapshot_limits values must be >= 0",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises when dbt config contains unknown key",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[dbt]
project_dir = "../dbt"
threads = 8
""".strip(),
            expected_error_fragment=r"dbt contains unknown key\(s\): threads",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises targeted error for removed dbt defer clone config",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[dbt]
defer_clone_from = true
""".strip(),
            expected_error_fragment=r"option\(s\) were removed: defer_clone_from",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises targeted error for removed dbt replay config",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[dbt]
replay_on_change = "full"
""".strip(),
            expected_error_fragment=r"option\(s\) were removed: replay_on_change",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises for unknown replay_on_change in defaults",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[defaults]
replay_on_change = "always"
""".strip(),
            expected_error_fragment=(
                r"\[defaults\] unknown replay_on_change 'always'; valid values: forward, full, "
                r"bounded-<duration>"
            ),
        ),
        LoadProjectConfigErrorTestCase(
            description="raises for unknown on_schema_change in path defaults",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[path_defaults.marts]
on_schema_change = "sync"
""".strip(),
            expected_error_fragment=(
                r"\[path_defaults.\"marts\"\] unknown on_schema_change 'sync'; valid values: "
                r"append_new_columns, fail, ignore, sync_all_columns"
            ),
        ),
        LoadProjectConfigErrorTestCase(
            description="raises targeted error for removed dbt production_ref reuse config",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[dbt.production_ref]
git_ref = "prod"
""".strip(),
            expected_error_fragment=r"reuse option\(s\) were removed: production_ref",
        ),
        LoadProjectConfigErrorTestCase(
            description="raises targeted error for removed legacy dbt reuse_from config",
            project_file_contents="""
name = "demo"
adapter = "duckdb"

[dbt.reuse_from]
git_ref = "prod"
""".strip(),
            expected_error_fragment=r"reuse option\(s\) were removed: reuse_from",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_project_config_file_when_loading_project_config_then_it_raises_clear_errors(
    test_case: LoadProjectConfigErrorTestCase,
    tmp_path: Path,
) -> None:
    project_file: Path = tmp_path / "sqlbuild_project.toml"
    project_file.write_text(test_case.project_file_contents, encoding="utf-8")

    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        load_project_config(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        LoadProjectConfigHelpTestCase(
            description="bounded replay without a valid duration in defaults",
            project_file_contents=(
                'name = "demo"\nadapter = "duckdb"\n\n[defaults]\nreplay_on_change = "bounded-x"\n'
            ),
            expected_error_fragment="replay_on_change 'bounded-x' has an invalid duration 'x'",
            expected_help=(
                "use a valid replay_on_change, set this in sqlbuild_project.toml:\n"
                "            [defaults]\n"
                '            replay_on_change = "bounded-14d"'
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unknown_change_policy_in_defaults_when_loading_then_help_shows_toml_line(
    test_case: LoadProjectConfigHelpTestCase, tmp_path: Path
) -> None:
    project_file: Path = tmp_path / "sqlbuild_project.toml"
    project_file.write_text(test_case.project_file_contents, encoding="utf-8")

    with pytest.raises(ProjectConfigError) as error_info:
        load_project_config(project_dir=tmp_path)

    assert test_case.expected_error_fragment in str(error_info.value)
    assert error_info.value.help == test_case.expected_help


@pytest.mark.parametrize(
    "test_case",
    [
        LoadNamedConnectionsTestCase(
            description="loads project named connection and scalar target reference",
            filename="sqlbuild_project.toml",
            contents='name = "demo"\nadapter = "snowflake"\n[connections.developer]\naccount = "acct"\ncustom = 7\n[targets.dev]\nconnection = "developer"',
            expected_connections={"developer": {"account": "acct", "custom": 7}},
            expected_connection_name="developer",
        ),
        LoadNamedConnectionsTestCase(
            description="loads local-only named connection and scalar target reference",
            filename="sqlbuild_local.toml",
            contents='[connections.local]\ndatabase = "local.duckdb"\n[targets.dev]\nconnection = "local"',
            expected_connections={"local": {"database": "local.duckdb"}},
            expected_connection_name="local",
        ),
        LoadNamedConnectionsTestCase(
            description="preserves legacy inline target connection mapping",
            filename="sqlbuild_project.toml",
            contents='name = "demo"\nadapter = "duckdb"\n[targets.dev.connection]\ndatabase = "legacy.duckdb"',
            expected_connections={},
            expected_connection_name=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_named_connection_syntax_when_loading_then_preserves_expected_shape(
    test_case: LoadNamedConnectionsTestCase, tmp_path: Path
) -> None:
    (tmp_path / test_case.filename).write_text(test_case.contents, encoding="utf-8")

    config: ProjectConfig | LocalConfig = {
        "sqlbuild_project.toml": load_project_config,
        "sqlbuild_local.toml": load_local_config,
    }[test_case.filename](project_dir=tmp_path)

    assert config.connections == test_case.expected_connections
    assert config.targets["dev"].connection_name == test_case.expected_connection_name


@pytest.mark.parametrize(
    "test_case",
    [
        LoadProjectConfigErrorTestCase(
            description="raises clear error when project config is missing",
            project_file_contents="",
            expected_error_fragment="Project config not found",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_missing_project_config_file_when_loading_project_config_then_it_raises_clear_error(
    test_case: LoadProjectConfigErrorTestCase,
    tmp_path: Path,
) -> None:
    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        load_project_config(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        LoadProjectConfigErrorTestCase(
            description="raises clear error when legacy project config is present",
            project_file_contents="name: demo\nadapter: duckdb\n",
            expected_error_fragment="sqlbuild_project.yml is no longer supported",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_legacy_project_config_file_when_loading_project_config_then_it_raises_clear_error(
    test_case: LoadProjectConfigErrorTestCase,
    tmp_path: Path,
) -> None:
    project_file: Path = tmp_path / "sqlbuild_project.yml"
    project_file.write_text(test_case.project_file_contents, encoding="utf-8")

    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        load_project_config(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        LoadLocalConfigErrorTestCase(
            description="raises when local target is not a non empty string",
            local_file_contents="target = 123\n",
            expected_error_fragment="Expected 'target' to be a non-empty string when provided",
        ),
        LoadLocalConfigErrorTestCase(
            description="raises when local vars contain non string value",
            local_file_contents="""
[vars]
user = 123
""".strip(),
            expected_error_fragment="expected string value for 'user'",
        ),
        LoadLocalConfigErrorTestCase(
            description="raises when local connection is not a mapping",
            local_file_contents='connection = "local.duckdb"\n',
            expected_error_fragment="Expected 'connection' to be a mapping when provided",
        ),
        LoadLocalConfigErrorTestCase(
            description="raises when local settings sql_analysis is not a boolean",
            local_file_contents="""
[settings]
sql_analysis = "no thanks"
""".strip(),
            expected_error_fragment="Expected 'sql_analysis' to be a boolean when provided",
        ),
        LoadLocalConfigErrorTestCase(
            description="raises when local settings concurrency is not an integer",
            local_file_contents="""
[settings]
concurrency = "many"
""".strip(),
            expected_error_fragment="Expected 'concurrency' to be an integer when provided",
        ),
        LoadLocalConfigErrorTestCase(
            description="raises when local settings contain unknown key",
            local_file_contents="""
[settings]
extra_concurrency = 8
""".strip(),
            expected_error_fragment=r"settings contains unknown key\(s\): extra_concurrency",
        ),
        LoadLocalConfigErrorTestCase(
            description="raises targeted error for removed local dbt defer clone config",
            local_file_contents="""
[dbt]
defer_clone_from = false
""".strip(),
            expected_error_fragment="dbt.*defer_clone_from was removed",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_local_config_file_when_loading_local_config_then_it_raises_clear_errors(
    test_case: LoadLocalConfigErrorTestCase,
    tmp_path: Path,
) -> None:
    local_file: Path = tmp_path / "sqlbuild_local.toml"
    local_file.write_text(test_case.local_file_contents, encoding="utf-8")

    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        load_local_config(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        LoadLocalConfigErrorTestCase(
            description="raises clear error when legacy local config is present",
            local_file_contents="environment: dev\n",
            expected_error_fragment="sqlbuild_local.yml is no longer supported",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_legacy_local_config_file_when_loading_local_config_then_it_raises_clear_error(
    test_case: LoadLocalConfigErrorTestCase,
    tmp_path: Path,
) -> None:
    local_file: Path = tmp_path / "sqlbuild_local.yml"
    local_file.write_text(test_case.local_file_contents, encoding="utf-8")

    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        load_local_config(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        CheckDeclaredColumnsConfigTestCase("omitted setting checks declared columns", "", True),
        CheckDeclaredColumnsConfigTestCase(
            "false leaves declared columns unchecked", "check_declared_columns = false", False
        ),
        CheckDeclaredColumnsConfigTestCase(
            "deprecated implicit mode maps to true", 'column_contract_mode = "implicit"', True
        ),
        CheckDeclaredColumnsConfigTestCase(
            "deprecated explicit mode maps to false", 'column_contract_mode = "explicit"', False
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_check_declared_columns_when_loading_project_then_setting_is_a_bool(
    test_case: CheckDeclaredColumnsConfigTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "demo"\nadapter = "duckdb"\n[settings]\n{test_case.settings_toml}\n',
        encoding="utf-8",
    )

    config: ProjectConfig = load_project_config(project_dir=tmp_path)

    assert config.settings.check_declared_columns is test_case.expected_checked


@pytest.mark.parametrize(
    "test_case",
    [
        CheckDeclaredColumnsConfigErrorTestCase(
            "unsupported deprecated mode is rejected",
            'column_contract_mode = "disabled"',
            "settings.column_contract_mode must be one of: implicit, explicit",
        ),
        CheckDeclaredColumnsConfigErrorTestCase(
            "non-boolean value is rejected",
            'check_declared_columns = "yes"',
            "check_declared_columns",
        ),
        CheckDeclaredColumnsConfigErrorTestCase(
            "setting both keys is rejected",
            'check_declared_columns = true\ncolumn_contract_mode = "implicit"',
            "settings cannot define both 'check_declared_columns' and deprecated "
            "'column_contract_mode'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_check_declared_columns_when_loading_project_then_error_is_raised(
    test_case: CheckDeclaredColumnsConfigErrorTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        f'name = "demo"\nadapter = "duckdb"\n[settings]\n{test_case.settings_toml}\n',
        encoding="utf-8",
    )

    with pytest.raises(ProjectConfigError, match=test_case.expected_error_fragment):
        load_project_config(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        DeepProjectConnectionTestCase(
            description="arrays nested fifty deep", depth=50, expected_depth=50
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_nested_connection_within_the_limit_when_loading_project_then_nesting_is_kept(
    test_case: DeepProjectConnectionTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "demo"\nadapter = "duckdb"\n[connection]\n'
        f"nested = {'[' * test_case.depth}1{']' * test_case.depth}\n",
        encoding="utf-8",
    )

    config: ProjectConfig = load_project_config(project_dir=tmp_path)

    nested: object = config.connection["nested"]
    depth: int = 0
    while isinstance(nested, list):
        nested = nested[0]
        depth += 1
    assert depth == test_case.expected_depth


@pytest.mark.parametrize(
    "test_case",
    [
        TooDeepProjectConnectionTestCase(
            description="arrays nested three hundred deep",
            depth=300,
            expected_error_fragment="max recursion depth",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_connection_nested_past_the_limit_when_loading_project_then_error_is_raised(
    test_case: TooDeepProjectConnectionTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "demo"\nadapter = "duckdb"\n[connection]\n'
        f"nested = {'[' * test_case.depth}1{']' * test_case.depth}\n",
        encoding="utf-8",
    )

    with pytest.raises(ProjectConfigError) as raised:
        load_project_config(project_dir=tmp_path)

    assert test_case.expected_error_fragment in str(raised.value)


@pytest.mark.parametrize(
    "test_case",
    [
        OversizedDurationTestCase(
            description="diff artifact TTL",
            section='[diff]\nquery_artifact_ttl = "{digits}h"\n',
            expected_error_fragment="Exceeds the limit (4300 digits)",
        ),
        OversizedDurationTestCase(
            description="lifecycle shutdown timeout",
            section='[sinks.lifecycle]\nshutdown_timeout = "{digits}s"\n',
            expected_error_fragment="value has 4301 digits",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_duration_beyond_int_digit_limit_when_loading_project_then_value_error_is_raised(
    test_case: OversizedDurationTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "demo"\nadapter = "duckdb"\n' + test_case.section.format(digits="1" * 4301),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match=re.escape(test_case.expected_error_fragment)):
        _ = load_project_config(project_dir=tmp_path)
