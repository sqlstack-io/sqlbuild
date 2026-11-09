from __future__ import annotations

import re
from collections.abc import Callable
from dataclasses import replace
from pathlib import Path
from typing import TYPE_CHECKING

import pytest

from sqlbuild._native import (
    AuthoredTimeTravelRetention,
    ClonePolicy,
    CompileModelConfig,
    DefaultsConfig,
    DiscoveredProjectInputs,
    LocalClonePolicy,
    LocalConfig,
    LocalTargetConfig,
    MaterializationDefaultsConfig,
    MaterializationRetentionDefaults,
    ProjectConfig,
    PythonHookEntry,
    ResolvedTimeTravelRetention,
    ResourceIdentityError,
    SpecConfigError,
    SqlHookEntry,
    TargetConfig,
    TimeTravelRetentionSource,
    build_compile_inputs,
    discover_project_inputs,
    resolve_target_config,
)
from tests.unit.src.sqlbuild.compiler.compile._helpers.config.helpers import build_config_natively
from tests.unit.src.sqlbuild.compiler.compile._helpers.helpers import (
    DUCKDB_COMPILE_ADAPTER_CONTEXT,
)
from tests.unit.src.sqlbuild.compiler.compile._test_helpers import (
    base_repo_files,
    build_external_sql_reference_resolver,
    compile_sql_test_authored_cte_names,
    compile_sql_test_expected_model_names,
    compile_sql_test_mock_model_names,
    compile_sql_test_mock_seed_names,
    compile_sql_test_mock_source_names,
    expected_or_actual,
)
from tests.unit.src.sqlbuild.compiler.compile._test_types import (
    AuthoredRetentionCompileTestCase,
    BuildCompileInputsDiagnosticTestCase,
    BuildCompileInputsErrorTestCase,
    BuildCompileInputsPythonHookValidationTestCase,
    BuildCompileInputsTestCase,
    BuildModelConfigHooksTestCase,
    BuildModelRetentionConfigTestCase,
    PreservedFunctionNamespaceTestCase,
    ResolveTargetConfigTestCase,
    SeedRefRegressionTestCase,
)

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView, FunctionInput


@pytest.mark.parametrize(
    "test_case",
    [
        BuildCompileInputsTestCase(
            description="attaches schema metadata to matching models and seeds and normalizes sources",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[connection]
path = "base.db"
warehouse = "default_wh"

[vars]
shared = "project"
project_only = "present"

[defaults]
materialized = "table"
schema = "analytics"
batch_size = "1h"
contract = "enforced"

[path_defaults]

[path_defaults.staging]
materialized = "view"
schema = "staging"

[path_defaults."staging/nested"]
schema = "nested"

[targets]

[targets.dev]

[targets.dev.connection]
warehouse = "dev_wh"

[targets.dev.vars]
shared = "environment"
env_only = "present"
""".strip()
                + "\n",
                "sqlbuild_local.toml": """
target = "dev"

[connection]
path = "local.db"

[settings]
sql_analysis = false
sql_validation = false
concurrency = 4

[vars]
shared = "local"
local_only = "present"
""".strip()
                + "\n",
                "models/staging/orders.sql": """
MODEL (description "Test model orders.",
  columns (
    order_id (type VARCHAR),
  ),
);

select 1
""".strip()
                + "\n",
                "models/staging/nested/orders_enriched.sql": """
MODEL (description "Test model orders_enriched.",
  materialized incremental,
  incremental_strategy delete_insert,
  cursor event_time,
  cursor_type timestamp,
  cursor_grain second,
  incremental_mode microbatch,
  microbatch_strategy rolling_window,
  batch_size 30m,
  contract none,
);

select __cursor_start  ( ) as batch_start, __cursor_end() as batch_end
""".strip()
                + "\n",
                "seeds/schema.yml": """
seeds:
  - name: country_codes
    description: Test seed country_codes.
    columns:
      - name: country_code
        type: VARCHAR
      - name: country_name
        type: VARCHAR
""".strip()
                + "\n",
                "seeds/country_codes.csv": "country_code,country_name\nUS,United States\n",
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    schema: public
    table: orders
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars={"shared": "cli", "cli_only": "present"},
            run_id=None,
            expected_model_schema_names=("orders_enriched", "orders"),
            expected_model_config_values=(
                {
                    "materialized": "incremental",
                    "schema": "nested",
                    "incremental_strategy": "delete_insert",
                    "cursor": "event_time",
                    "cursor_type": "timestamp",
                    "cursor_grain": "second",
                    "incremental_mode": "microbatch",
                    "microbatch_strategy": "rolling_window",
                    "batch_size": "30m",
                    "contract": "none",
                },
                {
                    "materialized": "view",
                    "schema": "staging",
                    "batch_size": "1h",
                    "contract": "enforced",
                },
            ),
            expected_model_query_sqls=(
                "select __cursor_start() as batch_start, __cursor_end() as batch_end",
                "select 1",
            ),
            expected_model_path_defaults=("staging/nested", "staging"),
            expected_seed_names=("country_codes",),
            expected_source_names=("raw_orders",),
            expected_effective_target_name="dev",
            expected_effective_connection={"path": "local.db", "warehouse": "dev_wh"},
            expected_effective_vars={
                "shared": "cli",
                "project_only": "present",
                "env_only": "present",
                "local_only": "present",
                "cli_only": "present",
            },
            expected_effective_sql_analysis=False,
            expected_effective_sql_validation=False,
            expected_effective_max_concurrency=4,
            expected_model_references=((), ()),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="allows models with no matching schema metadata",
            repo_files=base_repo_files()
            | {
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n"
            },
            selected_target=None,
            cli_vars={},
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=({},),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="validates incremental config columns against authored enforced contract",
            repo_files=base_repo_files()
            | {
                "models/orders.sql": """
MODEL (description "Test model orders.",
  materialized incremental,
  contract enforced,
  columns (
    id (type INTEGER),
    event_time (type TIMESTAMP),
  ),
  incremental_strategy delete_insert,
  cursor event_time,
  cursor_type timestamp,
  cursor_grain second,
  unique_key [id],
);

SELECT 1 AS id, CURRENT_TIMESTAMP AS event_time
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars={},
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=(
                {
                    "materialized": "incremental",
                    "contract": "enforced",
                    "incremental_strategy": "delete_insert",
                    "cursor": "event_time",
                    "cursor_type": "timestamp",
                    "cursor_grain": "second",
                    "unique_key": ["id"],
                },
            ),
            expected_model_query_sqls=("SELECT 1 AS id, CURRENT_TIMESTAMP AS event_time",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_column_metadata=(
                (
                    ("id", "INTEGER", None, ()),
                    ("event_time", "TIMESTAMP", None, ()),
                ),
            ),
            expected_model_references=((),),
            expected_audit_references=(),
            expected_diagnostic_codes=("P011",),
            expected_diagnostic_messages=(
                "incremental model 'orders' reads no inputs, so builds after the first "
                "cannot work out their cursor window",
            ),
        ),
        BuildCompileInputsTestCase(
            description="does not require snapshot generated validity columns in enforced contract",
            repo_files=base_repo_files()
            | {
                "models/customer_snapshot.sql": """
MODEL (description "Test model customer_snapshot.",
  materialized snapshot,
  contract enforced,
  columns (
    customer_id (type INTEGER),
    updated_at (type TIMESTAMP),
  ),
  unique_key [customer_id],
  snapshot_strategy timestamp,
  updated_at updated_at,
  valid_from_column effective_from,
  valid_to_column effective_to,
);

SELECT 1 AS customer_id, CURRENT_TIMESTAMP AS updated_at
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars={},
            run_id=None,
            expected_model_schema_names=("customer_snapshot",),
            expected_model_config_values=(
                {
                    "materialized": "snapshot",
                    "contract": "enforced",
                    "unique_key": ["customer_id"],
                    "snapshot_strategy": "timestamp",
                    "updated_at": "updated_at",
                    "valid_from_column": "effective_from",
                    "valid_to_column": "effective_to",
                },
            ),
            expected_model_query_sqls=("SELECT 1 AS customer_id, CURRENT_TIMESTAMP AS updated_at",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_column_metadata=(
                (
                    ("customer_id", "INTEGER", None, ()),
                    ("updated_at", "TIMESTAMP", None, ()),
                ),
            ),
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="prefers selected target over local and project defaults",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[connection]
path = "base.db"
warehouse = "default_wh"

[vars]
shared = "project"

[targets]

[targets.dev]

[targets.dev.connection]
warehouse = "dev_wh"

[targets.dev.vars]
shared = "dev"

[targets.prod]

[targets.prod.connection]
warehouse = "prod_wh"
role = "transformer"

[targets.prod.vars]
shared = "prod"
prod_only = "present"
""".strip()
                + "\n",
                "sqlbuild_local.toml": """
target = "dev"

[vars]
shared = "local"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target="prod",
            cli_vars={},
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=({},),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name="prod",
            expected_effective_connection={
                "path": "base.db",
                "warehouse": "prod_wh",
                "role": "transformer",
            },
            expected_effective_vars={"shared": "local", "prod_only": "present"},
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="prefers local target over project default when cli is absent",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "prod"

[connection]
path = "base.db"

[targets]

[targets.dev]

[targets.dev.connection]
warehouse = "dev_wh"

[targets.dev.vars]
active = "dev"

[targets.prod]

[targets.prod.connection]
warehouse = "prod_wh"

[targets.prod.vars]
active = "prod"
""".strip()
                + "\n",
                "sqlbuild_local.toml": """
target = "dev"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=({},),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name="dev",
            expected_effective_connection={"path": "base.db", "warehouse": "dev_wh"},
            expected_effective_vars={"active": "dev"},
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="returns no effective environment when none is configured anywhere",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[connection]
path = "base.db"

[vars]
project_only = "present"
""".strip()
                + "\n",
                "sqlbuild_local.toml": """
[vars]
local_only = "present"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target=None,
            cli_vars={"cli_only": "present"},
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=({},),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={"path": "base.db"},
            expected_effective_vars={
                "project_only": "present",
                "local_only": "present",
                "cli_only": "present",
            },
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="preserves project connection when environment has no connection override",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[connection]
path = "base.db"
warehouse = "default_wh"

[targets]

[targets.dev]

[targets.dev.vars]
active = "dev"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=({},),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name="dev",
            expected_effective_connection={"path": "base.db", "warehouse": "default_wh"},
            expected_effective_vars={"active": "dev"},
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="preserves non environment vars when selected target defines none",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[vars]
project_only = "present"

[targets]

[targets.dev]

[targets.dev.connection]
warehouse = "dev_wh"
""".strip()
                + "\n",
                "sqlbuild_local.toml": """
[vars]
local_only = "present"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target=None,
            cli_vars={"cli_only": "present"},
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=({},),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name="dev",
            expected_effective_connection={"warehouse": "dev_wh"},
            expected_effective_vars={
                "project_only": "present",
                "local_only": "present",
                "cli_only": "present",
            },
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="allows environment only connection when project connection is empty",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[targets]

[targets.dev]

[targets.dev.connection]
path = "env.db"
warehouse = "dev_wh"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=({},),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name="dev",
            expected_effective_connection={"path": "env.db", "warehouse": "dev_wh"},
            expected_effective_vars={},
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="allows env local and cli vars when project vars are empty",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[targets]

[targets.dev]

[targets.dev.vars]
shared = "env"
env_only = "present"
""".strip()
                + "\n",
                "sqlbuild_local.toml": """
[vars]
shared = "local"
local_only = "present"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target=None,
            cli_vars={"shared": "cli", "cli_only": "present"},
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=({},),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name="dev",
            expected_effective_connection={},
            expected_effective_vars={
                "shared": "cli",
                "env_only": "present",
                "local_only": "present",
                "cli_only": "present",
            },
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="preserves direct model header full refresh config",
            repo_files=base_repo_files()
            | {
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.', materialized incremental, incremental_strategy append, "
                    "full_refresh false);\n\nSELECT 1 AS id\n"
                ),
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=(
                {
                    "materialized": "incremental",
                    "incremental_strategy": "append",
                    "full_refresh": False,
                },
            ),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_query_sqls=("SELECT 1 AS id",),
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="applies default full refresh only to incremental models",
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\nadapter = "duckdb"\n\n[defaults]\nfull_refresh = false\n'
                ),
                "models/a_incremental.sql": (
                    "MODEL (description 'Test model a_incremental.', materialized incremental, incremental_strategy append);"
                    "\n\nSELECT 1 AS id\n"
                ),
                "models/b_view.sql": "MODEL (description 'Test model b_view.', materialized view);\n\nSELECT 1 AS id\n",
                "models/c_table.sql": "MODEL (description 'Test model c_table.', materialized table);\n\nSELECT 1 AS id\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("a_incremental", "b_view", "c_table"),
            expected_model_config_values=(
                {
                    "full_refresh": False,
                    "materialized": "incremental",
                    "incremental_strategy": "append",
                },
                {"materialized": "view"},
                {"materialized": "table"},
            ),
            expected_model_path_defaults=(None, None, None),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_query_sqls=(
                "SELECT 1 AS id",
                "SELECT 1 AS id",
                "SELECT 1 AS id",
            ),
            expected_model_references=((), (), ()),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="preserves default full refresh for templated incremental materialization",
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\nadapter = "duckdb"\n\n'
                    '[vars]\nmodel_kind = "incremental"\n\n'
                    '[defaults]\nmaterialized = "${model_kind}"\nfull_refresh = false\n'
                    'incremental_strategy = "append"\n'
                ),
                "models/orders.sql": "MODEL (description 'Test model orders.');\n\nSELECT 1 AS id\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=(
                {
                    "materialized": "incremental",
                    "full_refresh": False,
                    "incremental_strategy": "append",
                },
            ),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={"model_kind": "incremental"},
            expected_model_query_sqls=("SELECT 1 AS id",),
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="maps every supported project default into compile model config",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[defaults]
materialized = "incremental"
database = "analytics"
schema = "marts"
incremental_strategy = "merge"
incremental_mode = "microbatch"
microbatch_strategy = "rolling_window"
merge_exclude_columns = ["ingested_at"]
full_refresh = false
lookback = "1d"
batch_size = "1h"
replay_on_change = "bounded-30d"
row_diff_exclude_columns = ["loaded_at", "run_id"]

[defaults.row_diff_tolerances]

[defaults.row_diff_tolerances.by_column]

[defaults.row_diff_tolerances.by_column.revenue]
absolute = 0.01
""".strip()
                + "\n",
                "models/staging/orders.sql": (
                    "MODEL (description 'Test model orders.',\n  unique_key [order_id],"
                    "\n  cursor event_time,"
                    "\n  cursor_type timestamp,"
                    "\n  cursor_grain second,"
                    "\n);\n\nselect 1\n"
                ),
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=(
                {
                    "materialized": "incremental",
                    "database": "analytics",
                    "schema": "marts",
                    "incremental_strategy": "merge",
                    "incremental_mode": "microbatch",
                    "microbatch_strategy": "rolling_window",
                    "merge_exclude_columns": ("ingested_at",),
                    "full_refresh": False,
                    "unique_key": ["order_id"],
                    "cursor": "event_time",
                    "cursor_type": "timestamp",
                    "cursor_grain": "second",
                    "lookback": "1d",
                    "batch_size": "1h",
                    "replay_on_change": "bounded-30d",
                    "row_diff_exclude_columns": ("loaded_at", "run_id"),
                    "row_diff_tolerances": {
                        "by_column": {"revenue": {"absolute": 0.01}},
                    },
                },
            ),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="maps append cursor inclusive from project defaults",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[defaults]
materialized = "incremental"
incremental_strategy = "append"
append_cursor_inclusive = false
""".strip()
                + "\n",
                "models/staging/orders.sql": (
                    "MODEL (description 'Test model orders.',\n  cursor event_time,"
                    "\n  cursor_type timestamp,"
                    "\n  cursor_grain second,"
                    "\n);\n\nselect 1\n"
                ),
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=(
                {
                    "materialized": "incremental",
                    "incremental_strategy": "append",
                    "append_cursor_inclusive": False,
                    "cursor": "event_time",
                    "cursor_type": "timestamp",
                    "cursor_grain": "second",
                },
            ),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_references=((),),
            expected_audit_references=(),
            expected_diagnostic_codes=("P011",),
            expected_diagnostic_messages=(
                "incremental model 'orders' reads no inputs, so builds after the first "
                "cannot work out their cursor window",
            ),
        ),
        BuildCompileInputsTestCase(
            description="merges row diff config from project defaults and model header",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[defaults]
row_diff_exclude_columns = ["loaded_at"]

[defaults.row_diff_tolerances]

[defaults.row_diff_tolerances.by_type]

[defaults.row_diff_tolerances.by_type.float]
relative = 0.0001

[defaults.row_diff_tolerances.by_type.integer]
absolute = 1

[defaults.row_diff_tolerances.by_column]

[defaults.row_diff_tolerances.by_column.revenue]
absolute = 0.01
""".strip()
                + "\n",
                "models/staging/orders.sql": """
MODEL (description "Test model orders.",
  row_diff_exclude_columns [run_id, loaded_at],
  row_diff_tolerances (
    by_type (
      float (
        relative 0.00001,
      ),
    ),
    by_column (
      conversion_rate (
        relative 0.001,
        absolute 0.0001,
      ),
    ),
  ),
);

select 1
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=(
                {
                    "row_diff_exclude_columns": ("loaded_at", "run_id"),
                    "row_diff_tolerances": {
                        "by_type": {
                            "float": {"relative": 0.00001},
                            "integer": {"absolute": 1},
                        },
                        "by_column": {
                            "revenue": {"absolute": 0.01},
                            "conversion_rate": {
                                "relative": 0.001,
                                "absolute": 0.0001,
                            },
                        },
                    },
                },
            ),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="expands vars env connection model config and environment overrides",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[connection]
path = "/tmp/${user}.db"
warehouse = "${schema_prefix}_wh"

[vars]
user = "${ENV:USER}"
schema_prefix = "analytics_${user}"

[defaults]
database = "${schema_prefix}"
schema = "marts"
replay_on_change = "${ENV:BACKFILL_POLICY}"
row_diff_exclude_columns = ["${schema_prefix}_loaded_at"]

[targets]

[targets.dev]
database = "dev_${CTX:model.database}"
schema = "dev_${ENV:USER}_${CTX:model.schema}"
""".strip()
                + "\n",
                "models/staging/orders.sql": """
MODEL (description "Test model orders.",
  materialized incremental,
  incremental_strategy append,
  alias '${CTX:model.name}_${CTX:run.target}',
  config (
    cluster_by ['${schema_prefix}_day'],
    run_label '${CTX:run.id}',
    logical_alias '${CTX:model.alias}',
    target_table '${CTX:destination.table}',
    target_schema '${CTX:destination.schema}',
    target_database '${CTX:destination.database}',
    target_qualified '${CTX:destination.qualified}',
  ),
);

select 1
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id="run_123",
            expected_model_schema_names=("orders",),
            expected_model_config_values=(
                {
                    "database": "dev_analytics_kevin",
                    "schema": "dev_kevin_marts",
                    "materialized": "incremental",
                    "incremental_strategy": "append",
                    "replay_on_change": "bounded-30d",
                    "row_diff_exclude_columns": ("analytics_kevin_loaded_at",),
                    "alias": "orders_dev",
                    "config": {
                        "cluster_by": ["analytics_kevin_day"],
                        "run_label": "run_123",
                        "logical_alias": "orders_dev",
                        "target_table": "orders_dev",
                        "target_schema": "dev_kevin_marts",
                        "target_database": "dev_analytics_kevin",
                        "target_qualified": "dev_analytics_kevin.dev_kevin_marts.orders_dev",
                    },
                },
            ),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name="dev",
            expected_effective_connection={
                "path": "/tmp/kevin.db",
                "warehouse": "analytics_kevin_wh",
            },
            expected_effective_vars={
                "user": "kevin",
                "schema_prefix": "analytics_kevin",
            },
            environment_variables={"USER": "kevin", "BACKFILL_POLICY": "bounded-30d"},
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="resolves run context before late target context",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "ci"

[targets]

[targets.ci]
database = "db_${CTX:run.target}"
schema = "schema_${CTX:run.id}"
""".strip()
                + "\n",
                "models/staging/orders.sql": """
MODEL (description "Test model orders.",
  alias 'orders_${CTX:run.target}',
  config (
    run_label '${CTX:run.id}',
    target_qualified '${CTX:destination.qualified}',
  ),
);

select 1
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id="run_123",
            expected_model_schema_names=("orders",),
            expected_model_config_values=(
                {
                    "database": "db_ci",
                    "schema": "schema_run_123",
                    "alias": "orders_ci",
                    "config": {
                        "run_label": "run_123",
                        "target_qualified": "db_ci.schema_run_123.orders_ci",
                    },
                },
            ),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name="ci",
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="keeps default model context distinct across empty headers",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[defaults]
schema = "${CTX:model.name}_schema"
""".strip()
                + "\n",
                "models/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
                "models/customers.sql": "MODEL (description 'Test model customers.');\n\nselect 2\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("customers", "orders"),
            expected_model_config_values=(
                {"schema": "customers_schema"},
                {"schema": "orders_schema"},
            ),
            expected_model_query_sqls=("select 2", "select 1"),
            expected_model_path_defaults=(None, None),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_references=((), ()),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="expands helper functions in config interpolation",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[defaults]
materialized = "incremental"
incremental_strategy = "append"
database = "${if(ENV:CI, 'ci_db', 'dev_db')}"
schema = "${coalesce(ENV:CUSTOM_SCHEMA, 'fallback_schema')}"
append_cursor_inclusive = "${if(eq(ENV:APPEND_INCLUSIVE, '0'), false, true)}"
""".strip()
                + "\n",
                "models/staging/orders.sql": """
MODEL (description "Test model orders.",
  cursor event_time,
  cursor_type timestamp,
  cursor_grain second,
  alias '${if(eq(CTX:run.id, "run_123"), "orders_dev", "orders_prod")}',
);

select 1
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id="run_123",
            expected_model_schema_names=("orders",),
            expected_model_config_values=(
                {
                    "materialized": "incremental",
                    "incremental_strategy": "append",
                    "database": "ci_db",
                    "schema": "fallback_schema",
                    "append_cursor_inclusive": False,
                    "cursor": "event_time",
                    "cursor_type": "timestamp",
                    "cursor_grain": "second",
                    "alias": "orders_dev",
                },
            ),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            environment_variables={"CI": "1", "APPEND_INCLUSIVE": "0"},
            expected_model_references=((),),
            expected_audit_references=(),
            expected_diagnostic_codes=("P011",),
            expected_diagnostic_messages=(
                "incremental model 'orders' reads no inputs, so builds after the first "
                "cannot work out their cursor window",
            ),
        ),
        BuildCompileInputsTestCase(
            description="expands config templates in model header metadata fields",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[settings]
default_audit_severity = "warn"
sql_validation = false

[vars]
model_schema = "marts"
tag_name = "finance"
id_type = "INTEGER"
source_name = "orders_api"
valid_order_id = "1"
""".strip()
                + "\n",
                "models/marts/orders.sql": """
MODEL (
  schema "${model_schema}",
  alias "orders_${ENV:ENV_NAME}",
  description "Orders for ${ENV:ENV_NAME}",
  tags ["orders", "${tag_name}"],
  columns (
    order_id (
      type ${id_type},
      description "Order id from ${source_name}",
      audits [accepted_values (values ["${valid_order_id}"])],
    ),
  ),
);

select 1 as order_id
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_schema_descriptions=("Orders for dev",),
            expected_model_column_metadata=(
                (
                    (
                        "order_id",
                        "INTEGER",
                        "Order id from orders_api",
                        (("accepted_values", {"values": ["1"]}),),
                    ),
                ),
            ),
            expected_model_config_values=(
                {"schema": "marts", "alias": "orders_dev", "tags": ["orders", "finance"]},
            ),
            expected_model_query_sqls=("select 1 as order_id",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_audit_sql_bodies=(
                "SELECT order_id\n"
                'FROM __ref("orders")\n'
                "WHERE order_id IS NOT NULL\n"
                "  AND order_id NOT IN ('1')",
            ),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={
                "model_schema": "marts",
                "tag_name": "finance",
                "id_type": "INTEGER",
                "source_name": "orders_api",
                "valid_order_id": "1",
            },
            environment_variables={"ENV_NAME": "dev"},
            expected_effective_sql_analysis=False,
            expected_effective_sql_validation=False,
            expected_model_references=((),),
            expected_audit_references=((("ref", "orders"),),),
        ),
        BuildCompileInputsTestCase(
            description="supports multi hop var expansion and preserve environment overrides",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[vars]
base = "analytics"
stage_one = "${base}_team"
stage_two = "${stage_one}_prod"

[defaults]
database = "${stage_two}"
schema = "marts"

[path_defaults]

[path_defaults.staging]
alias = "${stage_two}_orders"

[targets]

[targets.dev]
database = "preserve"
schema = "preserve"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=(
                {
                    "database": "analytics_team_prod",
                    "schema": "marts",
                    "alias": "analytics_team_prod_orders",
                },
            ),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=("staging",),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name="dev",
            expected_effective_connection={},
            expected_effective_vars={
                "base": "analytics",
                "stage_one": "analytics_team",
                "stage_two": "analytics_team_prod",
            },
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="expands macros in model query and sql interpolation in hook strings",
            repo_files=base_repo_files()
            | {
                "macros/python/common.py": """
def project_columns(ctx) -> str:
    optional_suffix = "" if ctx.vars["optional_suffix"] is None else ctx.vars["optional_suffix"]
    return (
        "order_id, customer_id, "
        f"'{ctx.vars['run_label']}' AS label, "
        f"'{ctx.vars['grants']['role']}' AS role, "
        f"'{optional_suffix}' AS suffix"
    )


def grant_select(target: str) -> str:
    return f"INSERT INTO audit.read_grants VALUES ('{target}', 'analyst_role')"

""".strip()
                + "\n",
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[settings]
default_audit_severity = "warn"

[defaults]
schema = "marts"

[targets]

[targets.dev]
database = "analytics"
""".strip()
                + "\n",
                "models/staging/orders.sql": """
MODEL (description "Test model orders.",
  alias orders_dev,
  post_hooks [inline_sql('@py.grant_select("@@CTX:destination.qualified")')],
);

select @py.project_columns() from __source("raw_orders")
""".strip()
                + "\n",
                "tests/unit/orders_test.sql": """
TEST ();

WITH
__source__raw_orders AS (
  SELECT @py.project_columns()
  FROM raw_orders
),
__expected__orders AS (
  SELECT @py.project_columns()
)
SELECT 1
""".strip()
                + "\n",
                "audits/singular/orders.sql": """
AUDIT ();

SELECT @py.project_columns() FROM __source("raw_orders") JOIN __ref("orders") USING (order_id)
""".strip()
                + "\n",
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    table: orders
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars={
                "run_label": "cli_macro",
                "grants": {"role": "analyst"},
                "optional_suffix": None,
            },
            run_id="run_123",
            expected_diagnostic_codes=(),
            expected_diagnostic_messages=(),
            expected_model_schema_names=("orders",),
            expected_model_config_values=(
                {
                    "schema": "marts",
                    "alias": "orders_dev",
                    "database": "analytics",
                    "post_hooks": [
                        SqlHookEntry(
                            statement=(
                                "INSERT INTO audit.read_grants"
                                " VALUES ('analytics.marts.orders_dev', 'analyst_role')"
                            )
                        )
                    ],
                },
            ),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=("raw_orders",),
            expected_test_sql_bodies=(
                """
WITH
__source__raw_orders AS (
  SELECT order_id, customer_id, 'cli_macro' AS label, 'analyst' AS role, '' AS suffix
  FROM raw_orders
),
__expected__orders AS (
  SELECT order_id, customer_id, 'cli_macro' AS label, 'analyst' AS role, '' AS suffix
)
SELECT 1
""".strip(),
            ),
            expected_test_authored_cte_names=(("__source__raw_orders",),),
            expected_test_mock_model_names=((),),
            expected_test_mock_source_names=(("raw_orders",),),
            expected_test_mock_seed_names=((),),
            expected_test_expected_model_names=(("orders",),),
            expected_audit_sql_bodies=(
                "SELECT order_id, customer_id, 'cli_macro' AS label, 'analyst' AS role, "
                '\'\' AS suffix FROM __source("raw_orders") JOIN __ref("orders") USING (order_id)',
            ),
            expected_effective_target_name="dev",
            expected_effective_connection={},
            expected_effective_vars={
                "run_label": "cli_macro",
                "grants": {"role": "analyst"},
                "optional_suffix": None,
            },
            expected_model_query_sqls=(
                "select order_id, customer_id, 'cli_macro' AS label, 'analyst' AS role, "
                "'' AS suffix from __source(\"raw_orders\")",
            ),
            expected_model_references=((("source", "raw_orders"),),),
            expected_audit_references=((("source", "raw_orders"), ("ref", "orders")),),
        ),
        BuildCompileInputsTestCase(
            description="accepts dbt source and seed SQL test fixtures from manifest",
            repo_files=base_repo_files()
            | {
                "dbt/target/manifest.json": """
{
  "nodes": {
    "seed.analytics.countries": {
      "unique_id": "seed.analytics.countries",
      "resource_type": "seed",
      "package_name": "analytics",
      "name": "countries",
      "relation_name": "analytics.countries"
    }
  },
  "sources": {
    "source.analytics.raw.orders": {
      "unique_id": "source.analytics.raw.orders",
      "resource_type": "source",
      "package_name": "analytics",
      "source_name": "raw",
      "name": "orders",
      "relation_name": "raw.orders"
    }
  },
  "macros": {}
}
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
                "tests/unit/orders_test.sql": """
TEST ();

WITH
__source__raw__orders AS (
  SELECT 1 AS order_id
),
__seed__countries AS (
  SELECT 'US' AS country_code
),
__expected__orders AS (
  SELECT 1 AS order_id
)
SELECT 1
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=({},),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_query_sqls=("select 1",),
            expected_test_sql_bodies=(
                """
WITH
__source__raw__orders AS (
  SELECT 1 AS order_id
),
__seed__countries AS (
  SELECT 'US' AS country_code
),
__expected__orders AS (
  SELECT 1 AS order_id
)
SELECT 1
""".strip(),
            ),
            expected_test_authored_cte_names=(("__source__raw__orders", "__seed__countries"),),
            expected_test_mock_model_names=((),),
            expected_test_mock_source_names=(("raw__orders",),),
            expected_test_mock_seed_names=(("countries",),),
            expected_test_expected_model_names=(("orders",),),
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="applies sql interpolation across all authored sql text fields",
            repo_files=base_repo_files()
            | {
                "macros/python/common.py": """
def source_columns() -> str:
    return "1 AS id"
""".strip()
                + "\n",
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[settings]
sql_validation = false
default_audit_severity = "warn"

[vars]
raw_database = "raw_db"
raw_schema = "analytics_raw"
domain = "example.com"
audit_schema = "audit"
min_amount = "100"
""".strip()
                + "\n",
                "models/orders.sql": """
MODEL (description "Test model orders.",
  pre_hooks [inline_sql("insert into @@audit_schema.load_log select '@@ENV:USER_NAME'")],
  columns (order_id (audits [source_filter])),
);

SELECT *
FROM @@raw_schema.orders
WHERE email LIKE '%@@domain'
  AND event_date >= CURRENT_DATE
""".strip()
                + "\n",
                "functions/sql/is_large_order.sql": """
FUNCTION (description "Test function is_large_order.",
  arguments (amount INTEGER),
  returns BOOLEAN,
);

amount > @@min_amount AND '@@ENV:USER_NAME' = 'runner'
""".strip()
                + "\n",
                "tests/unit/orders_test.sql": """
TEST ();

WITH
__ref__orders AS (
  SELECT * FROM @@raw_schema.orders WHERE loaded_by = '@@ENV:USER_NAME'
),
__expected__orders AS (
  SELECT 1 AS id
)
SELECT 1
""".strip()
                + "\n",
                "audits/singular/orders.sql": """
AUDIT ();

SELECT * FROM __ref("orders") JOIN __source("raw_orders") USING (id)
WHERE loaded_by = '@@ENV:USER_NAME'
""".strip()
                + "\n",
                "audits/generic/source_filter.sql": """
AUDIT ();

SELECT @column
FROM @relation
WHERE @column IS NOT NULL
  AND source_system = '@@ENV:SOURCE_SYSTEM'
""".strip()
                + "\n",
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    expression: |
      SELECT @py.source_columns(), '@@ENV:USER_NAME' AS loaded_by
      FROM @@raw_schema.raw_orders
  - name: raw_table
    description: Test source raw_table.
    database: ${raw_database}
    schema: ${raw_schema}
    table: orders_${ENV:USER_NAME}
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=(
                {
                    "pre_hooks": [
                        SqlHookEntry(statement="insert into audit.load_log select 'runner'")
                    ]
                },
            ),
            expected_model_query_sqls=(
                "SELECT *\n"
                "FROM analytics_raw.orders\n"
                "WHERE email LIKE '%example.com'\n"
                "  AND event_date >= CURRENT_DATE",
            ),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=("raw_orders", "raw_table"),
            expected_source_expressions=(
                "SELECT 1 AS id, 'runner' AS loaded_by\nFROM analytics_raw.raw_orders\n",
                None,
            ),
            expected_source_relations=(
                (None, None, None),
                ("raw_db", "analytics_raw", "orders_runner"),
            ),
            expected_test_sql_bodies=(
                "WITH\n"
                "__ref__orders AS (\n"
                "  SELECT * FROM analytics_raw.orders WHERE loaded_by = 'runner'\n"
                "),\n"
                "__expected__orders AS (\n"
                "  SELECT 1 AS id\n"
                ")\n"
                "SELECT 1",
            ),
            expected_test_authored_cte_names=(("__ref__orders",),),
            expected_test_mock_model_names=(("orders",),),
            expected_test_mock_source_names=((),),
            expected_test_mock_seed_names=((),),
            expected_test_expected_model_names=(("orders",),),
            expected_audit_sql_bodies=(
                'SELECT * FROM __ref("orders") JOIN __source("raw_orders") USING (id)\n'
                "WHERE loaded_by = 'runner'",
                "SELECT order_id\n"
                'FROM __ref("orders")\n'
                "WHERE order_id IS NOT NULL\n"
                "  AND source_system = 'crm'",
            ),
            expected_sql_function_names=("is_large_order",),
            expected_sql_function_arguments=((("amount", "INTEGER"),),),
            expected_sql_function_returns=("BOOLEAN",),
            expected_sql_function_return_columns=((),),
            expected_sql_function_body_sqls=("amount > 100 AND 'runner' = 'runner'",),
            expected_sql_function_databases=(None,),
            expected_sql_function_schemas=(None,),
            expected_sql_function_languages=("sql",),
            expected_sql_function_runtime_versions=(None,),
            expected_sql_function_entry_points=(None,),
            expected_sql_function_packages=((),),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={
                "raw_database": "raw_db",
                "raw_schema": "analytics_raw",
                "domain": "example.com",
                "audit_schema": "audit",
                "min_amount": "100",
            },
            expected_effective_sql_analysis=False,
            expected_effective_sql_validation=False,
            expected_model_references=((),),
            expected_audit_references=(
                (("ref", "orders"), ("source", "raw_orders")),
                (("ref", "orders"),),
            ),
            environment_variables={
                "USER_NAME": "runner",
                "SOURCE_SYSTEM": "crm",
            },
        ),
        BuildCompileInputsTestCase(
            description="expands typed SQL hooks without expanding Python hook kwargs",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[defaults]
schema = "marts"

[targets]

[targets.dev]
database = "analytics"
""".strip()
                + "\n",
                "models/orders.sql": """
MODEL (description "Test model orders.",
  alias orders_dev,
  post_hooks [
    inline_sql("select '@@CTX:destination.qualified' as target"),
    inline_sql("create macro @@CTX:destination.database.@@CTX:destination.schema.reconstruct_book() as 1"),
    python("notify", message: "@@CTX:destination.qualified"),
  ],
);

SELECT 1 AS id
""".strip()
                + "\n",
                "hooks/python/notifications.py": """
from sqlbuild.hooks import hook


@hook
def notify(ctx, message):
    '''Test hook notify.'''
    return None
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_diagnostic_codes=(),
            expected_diagnostic_messages=(),
            expected_model_schema_names=("orders",),
            expected_model_config_values=(
                {
                    "schema": "marts",
                    "alias": "orders_dev",
                    "database": "analytics",
                    "post_hooks": [
                        SqlHookEntry(statement="select 'analytics.marts.orders_dev' as target"),
                        SqlHookEntry(
                            statement="create macro analytics.marts.reconstruct_book() as 1"
                        ),
                        PythonHookEntry(
                            name="notify",
                            kwargs={"message": "@@CTX:destination.qualified"},
                        ),
                    ],
                },
            ),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name="dev",
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_query_sqls=("SELECT 1 AS id",),
            expected_model_references=((),),
        ),
        BuildCompileInputsTestCase(
            description="resolves named SQL hooks with arguments and model context",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[defaults]
database = "analytics"
schema = "marts"
""".strip()
                + "\n",
                "models/orders.sql": """
MODEL (description "Test model orders.",
  post_hooks [sql("record_access", role: "O'Brien")],
);

SELECT 1 AS id
""".strip()
                + "\n",
                "hooks/sql/record_access.sql": """
HOOK (description "Record model access");

SELECT @'role' AS role, '@@CTX:destination.qualified' AS relation_name
FROM @@CTX:destination.database.@@CTX:destination.schema.access_log
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=(
                {
                    "database": "analytics",
                    "schema": "marts",
                    "post_hooks": [
                        SqlHookEntry(
                            statement=(
                                "SELECT 'O''Brien' AS role, 'analytics.marts.orders' AS relation_name\n"
                                "FROM analytics.marts.access_log"
                            ),
                            name="record_access",
                            relative_path=Path("hooks/sql/record_access.sql"),
                            definition_sql=(
                                "SELECT @'role' AS role, "
                                "'@@CTX:destination.qualified' AS relation_name\n"
                                "FROM @@CTX:destination.database."
                                "@@CTX:destination.schema.access_log"
                            ),
                            kwargs={"role": "O'Brien"},
                            description="Record model access",
                        )
                    ],
                },
            ),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_query_sqls=("SELECT 1 AS id",),
            expected_model_references=((),),
        ),
        BuildCompileInputsTestCase(
            description="validates Python hook kwargs while allowing defaults and variadic kwargs",
            repo_files=base_repo_files()
            | {
                "models/orders.sql": """
MODEL (description "Test model orders.",
  pre_hooks [
    python("defaulted", channel: "alerts"),
    python("flexible", extra: "value"),
    python("context_named", message: "ok"),
    python("optional_positional_only"),
  ],
);

SELECT 1 AS id
""".strip()
                + "\n",
                "hooks/python/notifications.py": """
from sqlbuild.hooks import hook


@hook
def defaulted(context, channel, message="ready"):
    '''Test hook defaulted.'''
    return None


@hook
def flexible(ctx, **kwargs):
    '''Test hook flexible.'''
    return None


@hook
def context_named(hook_context, message):
    '''Test hook context_named.'''
    return None


@hook
def optional_positional_only(ctx, channel="alerts", /):
    '''Test hook optional_positional_only.'''
    return None
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=(
                {
                    "pre_hooks": [
                        PythonHookEntry(name="defaulted", kwargs={"channel": "alerts"}),
                        PythonHookEntry(name="flexible", kwargs={"extra": "value"}),
                        PythonHookEntry(name="context_named", kwargs={"message": "ok"}),
                        PythonHookEntry(name="optional_positional_only", kwargs={}),
                    ],
                },
            ),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_query_sqls=("SELECT 1 AS id",),
            expected_model_references=((),),
        ),
        BuildCompileInputsTestCase(
            description="preserves raw Python hook kwargs after discovered hook validation",
            repo_files=base_repo_files()
            | {
                "models/orders.sql": """
MODEL (description "Test model orders.",
  post_hooks [
    python("notify", message: "@@CTX:destination.qualified", macro_text: "@py.format_message()"),
  ],
);

SELECT 1 AS id
""".strip()
                + "\n",
                "hooks/python/notifications.py": """
from sqlbuild.hooks import hook


@hook
def notify(ctx, message, macro_text):
    '''Test hook notify.'''
    return None
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=(
                {
                    "post_hooks": [
                        PythonHookEntry(
                            name="notify",
                            kwargs={
                                "message": "@@CTX:destination.qualified",
                                "macro_text": "@py.format_message()",
                            },
                        ),
                    ],
                },
            ),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_query_sqls=("SELECT 1 AS id",),
            expected_model_references=((),),
        ),
        BuildCompileInputsTestCase(
            description="expands vars and macros in sql function bodies and headers",
            repo_files=base_repo_files()
            | {
                "macros/python/common.py": """
def status_match(column_name: str, status: str) -> str:
    return f"{column_name} = '{status}'"
""".strip()
                + "\n",
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[vars]
status_type = "VARCHAR"
return_type = "BOOLEAN"
cancelled_status = "'cancelled'"
udf_database = "analytics"
udf_schema = "udf_dev"
backfill_days = "30"

[targets]

[targets.dev]
database = "analytics_default"
schema = "default_schema"
""".strip()
                + "\n",
                "functions/sql/is_completed_order.sql": """
FUNCTION (description "Test function is_completed_order.",
  database ${udf_database},
  schema ${udf_schema},
  arguments (order_status ${status_type}),
  returns ${return_type},
);

@py.status_match("order_status", "completed") AND order_status <> @@cancelled_status
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=(),
            expected_model_config_values=(),
            expected_model_query_sqls=(),
            expected_model_path_defaults=(),
            expected_seed_names=(),
            expected_source_names=(),
            expected_sql_function_names=("is_completed_order",),
            expected_sql_function_arguments=((("order_status", "VARCHAR"),),),
            expected_sql_function_returns=("BOOLEAN",),
            expected_sql_function_return_columns=((),),
            expected_sql_function_body_sqls=(
                "order_status = 'completed' AND order_status <> 'cancelled'",
            ),
            expected_sql_function_databases=("analytics_default",),
            expected_sql_function_schemas=("default_schema",),
            expected_sql_function_logical_databases=("analytics",),
            expected_sql_function_logical_schemas=("udf_dev",),
            expected_sql_function_languages=("sql",),
            expected_sql_function_runtime_versions=(None,),
            expected_sql_function_entry_points=(None,),
            expected_sql_function_packages=((),),
            expected_effective_target_name="dev",
            expected_effective_connection={},
            expected_effective_vars={
                "status_type": "VARCHAR",
                "return_type": "BOOLEAN",
                "cancelled_status": "'cancelled'",
                "udf_database": "analytics",
                "udf_schema": "udf_dev",
                "backfill_days": "30",
            },
            expected_model_references=(),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="attaches table function metadata and accepts a managed model dependency",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[settings]
sql_validation = false
""".strip()
                + "\n",
                "models/fact_orders.sql": (
                    'MODEL (description "Test model fact_orders.");\n\nSELECT * FROM __table_fn("customer_orders")(7)\n'
                ),
                "functions/sql/customer_orders.sql": """
FUNCTION (description "Test function customer_orders.",
  arguments (p_customer_id INTEGER),
  returns table (
    order_id INTEGER,
    amount_cents INTEGER
  )
);

SELECT order_id, 100 AS amount_cents
FROM __ref("fact_orders")
WHERE customer_id = p_customer_id
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("fact_orders",),
            expected_model_config_values=({},),
            expected_model_query_sqls=('SELECT * FROM __table_fn("customer_orders")(7)',),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_sql_function_names=("customer_orders",),
            expected_sql_function_arguments=((("p_customer_id", "INTEGER"),),),
            expected_sql_function_returns=("TABLE",),
            expected_sql_function_return_columns=(
                (("order_id", "INTEGER"), ("amount_cents", "INTEGER")),
            ),
            expected_sql_function_body_sqls=(
                "SELECT order_id, 100 AS amount_cents\n"
                'FROM __ref("fact_orders")\n'
                "WHERE customer_id = p_customer_id",
            ),
            expected_sql_function_databases=(None,),
            expected_sql_function_schemas=(None,),
            expected_sql_function_languages=("sql",),
            expected_sql_function_runtime_versions=(None,),
            expected_sql_function_entry_points=(None,),
            expected_sql_function_packages=((),),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_effective_sql_analysis=False,
            expected_effective_sql_validation=False,
            expected_model_references=((("table_fn", "customer_orders"),),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="accepts commented qualified table function outputs without self aliases",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
                "functions/sql/customer_orders.sql": """
FUNCTION (description "Test function customer_orders.",
  returns table (
    order_id INTEGER,
    status VARCHAR
  )
);

SELECT
  orders.order_id,
  -- Current order status.
  orders.status
FROM (VALUES (1, 'open')) AS orders(order_id, status)
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=(),
            expected_model_config_values=(),
            expected_model_path_defaults=(),
            expected_seed_names=(),
            expected_source_names=(),
            expected_sql_function_names=("customer_orders",),
            expected_sql_function_arguments=((),),
            expected_sql_function_returns=("TABLE",),
            expected_sql_function_return_columns=(
                (("order_id", "INTEGER"), ("status", "VARCHAR")),
            ),
            expected_sql_function_body_sqls=(
                "SELECT\n"
                "  orders.order_id,\n"
                "  -- Current order status.\n"
                "  orders.status\n"
                "FROM (VALUES (1, 'open')) AS orders(order_id, status)",
            ),
            expected_sql_function_databases=(None,),
            expected_sql_function_schemas=(None,),
            expected_sql_function_languages=("sql",),
            expected_sql_function_runtime_versions=(None,),
            expected_sql_function_entry_points=(None,),
            expected_sql_function_packages=((),),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_references=(),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="expands macros across multi block tests and audits",
            repo_files=base_repo_files()
            | {
                "macros/python/common.py": """
def project_columns() -> str:
    return "order_id"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
                "tests/unit/orders_test.sql": """
TEST (name "first");

WITH
__ref__orders AS (
  SELECT @py.project_columns() FROM raw_orders
),
__expected__orders AS (
  SELECT @py.project_columns()
)
SELECT 1;

TEST (name "second");

WITH
__ref__orders AS (
  SELECT @py.project_columns() FROM raw_customers
),
__expected__orders AS (
  SELECT @py.project_columns()
)
SELECT 1
""".strip()
                + "\n",
                "audits/singular/orders.sql": """
AUDIT (name "first");

SELECT @py.project_columns() FROM __ref("orders") JOIN __source("raw_orders") USING (order_id);

AUDIT (name "second");

SELECT @py.project_columns() FROM __ref("orders") JOIN __source("raw_customers") USING (order_id)
""".strip()
                + "\n",
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    table: orders
  - name: raw_customers
    description: Test source raw_customers.
    table: customers
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=({},),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=("raw_orders", "raw_customers"),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_test_sql_bodies=(
                """
WITH
__ref__orders AS (
  SELECT order_id FROM raw_orders
),
__expected__orders AS (
  SELECT order_id
)
SELECT 1;
""".strip(),
                """
WITH
__ref__orders AS (
  SELECT order_id FROM raw_customers
),
__expected__orders AS (
  SELECT order_id
)
SELECT 1
""".strip(),
            ),
            expected_test_authored_cte_names=(
                ("__ref__orders",),
                ("__ref__orders",),
            ),
            expected_test_mock_model_names=(("orders",), ("orders",)),
            expected_test_mock_source_names=((), ()),
            expected_test_mock_seed_names=((), ()),
            expected_test_expected_model_names=(("orders",), ("orders",)),
            expected_audit_sql_bodies=(
                'SELECT order_id FROM __ref("orders") JOIN __source("raw_orders") USING (order_id);',
                'SELECT order_id FROM __ref("orders") JOIN __source("raw_customers") '
                "USING (order_id)",
            ),
            expected_model_references=((),),
            expected_audit_references=(
                (("ref", "orders"), ("source", "raw_orders")),
                (("ref", "orders"), ("source", "raw_customers")),
            ),
        ),
        BuildCompileInputsTestCase(
            description="discovers python sqlbuild udf metadata",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[vars]
status_type = "VARCHAR"
return_type = "BOOLEAN"
udf_schema = "udf_dev"

[targets]

[targets.dev]
schema = "default_schema"
""".strip()
                + "\n",
                "functions/python/is_completed_order.py": """
from sqlbuild.functions import udf

@udf(
    arguments={"order_status": "${status_type}"},
    returns="${return_type}",
    runtime_version="3.11",
    schema="${udf_schema}",
    packages=["faker"],
)
def main(order_status):
    '''Test function main.'''
    return order_status == "completed"
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=(),
            expected_model_config_values=(),
            expected_model_query_sqls=(),
            expected_model_path_defaults=(),
            expected_seed_names=(),
            expected_source_names=(),
            expected_sql_function_names=("is_completed_order",),
            expected_sql_function_arguments=((("order_status", "VARCHAR"),),),
            expected_sql_function_returns=("BOOLEAN",),
            expected_sql_function_return_columns=((),),
            expected_sql_function_body_sqls=(
                """
def main(order_status):
    \"\"\"Test function main.\"\"\"
    return order_status == 'completed'
""".strip(),
            ),
            expected_sql_function_databases=(None,),
            expected_sql_function_schemas=("default_schema",),
            expected_sql_function_logical_databases=(None,),
            expected_sql_function_logical_schemas=("udf_dev",),
            expected_sql_function_languages=("python",),
            expected_sql_function_runtime_versions=("3.11",),
            expected_sql_function_entry_points=("main",),
            expected_sql_function_packages=(("faker",),),
            expected_effective_target_name="dev",
            expected_effective_connection={},
            expected_effective_vars={
                "status_type": "VARCHAR",
                "return_type": "BOOLEAN",
                "udf_schema": "udf_dev",
            },
            expected_model_references=(),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="renders attached model and source audits from generic definitions",
            repo_files=base_repo_files()
            | {
                "models/marts/orders.sql": """
MODEL (description "Test model orders.",
  audits [model_not_null],
  columns (
    order_id (audits [column_not_null]),
  ),
);

select 1
""".strip()
                + "\n",
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    audits:
      - source_not_null
    columns:
      - name: order_id
        audits:
          - source_column_not_null
""".strip()
                + "\n",
                "audits/generic/model_not_null.sql": """
AUDIT ();

SELECT 1 FROM __ref("@model")
""".strip()
                + "\n",
                "audits/generic/column_not_null.sql": """
AUDIT ();

SELECT @column FROM __ref("@model") WHERE @column IS NULL
""".strip()
                + "\n",
                "audits/generic/source_not_null.sql": """
AUDIT ();

SELECT 1 FROM __source("@source")
""".strip()
                + "\n",
                "audits/generic/source_column_not_null.sql": """
AUDIT ();

SELECT @column FROM __source("@source") WHERE @column IS NULL
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=({},),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=("raw_orders",),
            expected_test_sql_bodies=(),
            expected_audit_sql_bodies=(
                'SELECT 1 FROM __ref("orders")',
                'SELECT order_id FROM __ref("orders") WHERE order_id IS NULL',
                'SELECT 1 FROM __source("raw_orders")',
                'SELECT order_id FROM __source("raw_orders") WHERE order_id IS NULL',
            ),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_references=((),),
            expected_audit_references=(
                (("ref", "orders"),),
                (("ref", "orders"),),
                (("source", "raw_orders"),),
                (("source", "raw_orders"),),
            ),
        ),
        BuildCompileInputsTestCase(
            description="renders built-in attached audits when no project generic definition exists",
            repo_files=base_repo_files()
            | {
                "models/marts/orders.sql": """
MODEL (description "Test model orders.",
  columns (
    order_id (nullable false, audits [not_null, unique]),
    status (audits [accepted_values (values ["placed", "completed"])]),
    customer_id (
      nullable true,
      audits [relationships (to __ref("customers"), field customer_id)],
    ),
  ),
);

select 1 as order_id, 'placed' as status, null as customer_id
""".strip()
                + "\n",
                "models/marts/customers.sql": "MODEL (description 'Test model customers.');\n\nselect 1 as customer_id\n",
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    columns:
      - name: order_id
        audits:
          - not_null
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("customers", "orders"),
            expected_model_config_values=({}, {}),
            expected_model_query_sqls=(
                "select 1 as customer_id",
                "select 1 as order_id, 'placed' as status, null as customer_id",
            ),
            expected_model_column_nullables=((), (False, None, True)),
            expected_model_path_defaults=(None, None),
            expected_seed_names=(),
            expected_source_names=("raw_orders",),
            expected_test_sql_bodies=(),
            expected_audit_sql_bodies=(
                'SELECT order_id\nFROM __ref("orders")\nWHERE order_id IS NULL',
                'SELECT order_id, COUNT(*) AS duplicate_count\nFROM __ref("orders")\n'
                "WHERE order_id IS NOT NULL\nGROUP BY order_id\nHAVING COUNT(*) > 1",
                'SELECT status\nFROM __ref("orders")\nWHERE status IS NOT NULL\n'
                "  AND status NOT IN ('placed', 'completed')",
                'SELECT customer_id\nFROM __ref("orders")\nWHERE customer_id IS NOT NULL\n'
                "  AND customer_id NOT IN (\n"
                "    SELECT customer_id\n"
                '    FROM __ref("customers")\n'
                "    WHERE customer_id IS NOT NULL\n"
                "  )",
                'SELECT order_id\nFROM __source("raw_orders")\nWHERE order_id IS NULL',
            ),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_references=((), ()),
            expected_audit_references=(
                (("ref", "orders"),),
                (("ref", "orders"),),
                (("ref", "orders"),),
                (("ref", "orders"), ("ref", "customers")),
                (("source", "raw_orders"),),
            ),
        ),
        BuildCompileInputsTestCase(
            description="skips generic audit definitions as direct executable audits",
            repo_files=base_repo_files()
            | {
                "models/marts/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
                "audits/generic/custom_check.sql": """
AUDIT ();

SELECT 1
""".strip()
                + "\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=({},),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_test_sql_bodies=(),
            expected_audit_sql_bodies=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="warns when project generic audit shadows built-in audit",
            repo_files=base_repo_files()
            | {
                "models/marts/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
                "audits/generic/not_null.sql": "AUDIT ();\n\nSELECT 1\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=({},),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_test_sql_bodies=(),
            expected_audit_sql_bodies=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_references=((),),
            expected_audit_references=(),
            expected_diagnostic_codes=("P003",),
            expected_diagnostic_messages=(
                "project audit 'not_null' overrides built-in audit 'not_null'",
            ),
        ),
        BuildCompileInputsTestCase(
            description="generates clickstate style run ids when none are provided",
            repo_files=base_repo_files()
            | {
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n"
            },
            selected_target=None,
            cli_vars=None,
            run_id=None,
            expected_model_schema_names=("orders",),
            expected_model_config_values=({},),
            expected_model_query_sqls=("select 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="allows invalid sql when project sql validation is disabled",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[settings]
sql_validation = false
""".strip()
                + "\n",
                "models/staging/broken.sql": "MODEL (description 'Test model broken.');\n\nSELEC id FROM (SELECT 1\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id="test_run",
            expected_model_schema_names=("broken",),
            expected_model_config_values=({},),
            expected_model_query_sqls=("SELEC id FROM (SELECT 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_effective_sql_analysis=False,
            expected_effective_sql_validation=False,
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="allows invalid sql when sql_analysis is disabled",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[settings]
sql_analysis = false
""".strip()
                + "\n",
                "models/staging/broken.sql": "MODEL (description 'Test model broken.');\n\nSELEC id FROM (SELECT 1\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id="test_run",
            expected_model_schema_names=("broken",),
            expected_model_config_values=({},),
            expected_model_query_sqls=("SELEC id FROM (SELECT 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_effective_sql_analysis=False,
            expected_effective_sql_validation=False,
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="allows invalid sql when no_sql_validation flag is set",
            repo_files=base_repo_files()
            | {
                "models/staging/broken.sql": "MODEL (description 'Test model broken.');\n\nSELEC id FROM (SELECT 1\n",
            },
            selected_target=None,
            cli_vars=None,
            run_id="test_run",
            no_sql_validation=True,
            expected_model_schema_names=("broken",),
            expected_model_config_values=({},),
            expected_model_query_sqls=("SELEC id FROM (SELECT 1",),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_references=((),),
            expected_audit_references=(),
        ),
        BuildCompileInputsTestCase(
            description="validates dbt refs from discovered manifest while preserving markers",
            repo_files=base_repo_files()
            | {
                "dbt/target/manifest.json": """
{
  "nodes": {
    "model.analytics.stg_orders": {
      "unique_id": "model.analytics.stg_orders",
      "resource_type": "model",
      "package_name": "analytics",
      "name": "stg_orders",
      "relation_name": "analytics.stg_orders"
    },
    "model.stripe.orders": {
      "unique_id": "model.stripe.orders",
      "resource_type": "model",
      "package_name": "stripe",
      "name": "orders",
      "relation_name": "stripe.orders"
    }
  }
}
""".strip()
                + "\n",
                "models/fact_orders.sql": (
                    'MODEL (description "Test model fact_orders.");\n\nselect * from __dbt_ref("stg_orders") '
                    'union all select * from __dbt_ref("stripe", "orders")\n'
                ),
            },
            selected_target=None,
            cli_vars=None,
            run_id="test_run",
            expected_model_schema_names=("fact_orders",),
            expected_model_config_values=({},),
            expected_model_query_sqls=(
                'select * from __dbt_ref("stg_orders") '
                'union all select * from __dbt_ref("stripe", "orders")',
            ),
            expected_model_path_defaults=(None,),
            expected_seed_names=(),
            expected_source_names=(),
            expected_effective_target_name=None,
            expected_effective_connection={},
            expected_effective_vars={},
            expected_model_references=((("dbt_ref", "stg_orders"), ("dbt_ref", "orders")),),
            expected_audit_references=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_discovered_inputs_when_building_compile_inputs_then_it_attaches_metadata(  # noqa: PLR0915
    test_case: BuildCompileInputsTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    target_name: str
    environment_value: str
    for target_name, environment_value in test_case.environment_variables.items():
        monkeypatch.setenv(target_name, environment_value)

    write_repo_files(tmp_path, test_case.repo_files)

    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    compile_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered_inputs,
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
        selected_target=test_case.selected_target,
        cli_vars=test_case.cli_vars,
        run_id=test_case.run_id,
        no_sql_validation=test_case.no_sql_validation,
        external_sql_reference_resolver=build_external_sql_reference_resolver(project_dir=tmp_path),
    )

    assert (
        tuple(
            getattr(model_input.schema_entry, "name", None)
            for model_input in compile_inputs.model_inputs
        )
        == test_case.expected_model_schema_names
    )
    assert (
        tuple(model_input.config.values for model_input in compile_inputs.model_inputs)
        == test_case.expected_model_config_values
    )
    assert (
        tuple(model_input.query_sql for model_input in compile_inputs.model_inputs)
        == test_case.expected_model_query_sqls
    )
    actual_model_column_nullables_list: list[tuple[bool | None, ...]] = []
    for model_input in compile_inputs.model_inputs:
        column_nullables: list[bool | None] = []
        for column in getattr(model_input.schema_entry, "columns", ()):
            column_nullables.append(column.nullable)
        actual_model_column_nullables_list.append(tuple(column_nullables))
    actual_model_column_nullables: tuple[tuple[bool | None, ...], ...] = tuple(
        actual_model_column_nullables_list
    )
    assert actual_model_column_nullables == expected_or_actual(
        test_case.expected_model_column_nullables, actual_model_column_nullables
    )
    actual_model_schema_descriptions: tuple[str | None, ...] = tuple(
        getattr(model_input.schema_entry, "description", None)
        for model_input in compile_inputs.model_inputs
    )
    assert actual_model_schema_descriptions == expected_or_actual(
        test_case.expected_model_schema_descriptions,
        actual_model_schema_descriptions,
    )
    model_column_metadata_list: list[
        tuple[
            tuple[
                str,
                str | None,
                str | None,
                tuple[tuple[str, dict[str, object]], ...],
            ],
            ...,
        ]
    ] = []
    for model_input in compile_inputs.model_inputs:
        column_metadata: list[
            tuple[str, str | None, str | None, tuple[tuple[str, dict[str, object]], ...]]
        ] = []
        for column in getattr(model_input.schema_entry, "columns", ()):
            audit_metadata: list[tuple[str, dict[str, object]]] = []
            for audit in column.audits:
                audit_metadata.append((audit.definition_name, audit.arguments))
            column_metadata.append(
                (
                    column.name,
                    column.type,
                    column.description,
                    tuple(audit_metadata),
                )
            )
        model_column_metadata_list.append(tuple(column_metadata))
    actual_model_column_metadata: tuple[
        tuple[
            tuple[
                str,
                str | None,
                str | None,
                tuple[tuple[str, dict[str, object]], ...],
            ],
            ...,
        ],
        ...,
    ] = tuple(model_column_metadata_list)
    assert actual_model_column_metadata == expected_or_actual(
        test_case.expected_model_column_metadata,
        actual_model_column_metadata,
    )
    assert (
        tuple(
            model_input.config.matched_path_default for model_input in compile_inputs.model_inputs
        )
        == test_case.expected_model_path_defaults
    )
    assert (
        tuple(seed_input.schema_entry.name for seed_input in compile_inputs.seed_inputs)
        == test_case.expected_seed_names
    )
    assert (
        tuple(source_input.source_entry.name for source_input in compile_inputs.source_inputs)
        == test_case.expected_source_names
    )
    actual_source_expressions: tuple[str | None, ...] = tuple(
        source_input.source_entry.expression for source_input in compile_inputs.source_inputs
    )
    assert actual_source_expressions == expected_or_actual(
        test_case.expected_source_expressions,
        actual_source_expressions,
    )
    actual_source_relations: tuple[tuple[str | None, str | None, str | None], ...] = tuple(
        (
            source_input.source_entry.database,
            source_input.source_entry.schema,
            source_input.source_entry.table,
        )
        for source_input in compile_inputs.source_inputs
    )
    assert actual_source_relations == expected_or_actual(
        test_case.expected_source_relations,
        actual_source_relations,
    )
    assert (
        tuple(test_input.sql_body for test_input in compile_inputs.test_inputs)
        == test_case.expected_test_sql_bodies
    )
    assert (
        tuple(
            compile_sql_test_authored_cte_names(test_input)
            for test_input in compile_inputs.test_inputs
        )
        == test_case.expected_test_authored_cte_names
    )
    assert (
        tuple(
            compile_sql_test_mock_model_names(test_input)
            for test_input in compile_inputs.test_inputs
        )
        == test_case.expected_test_mock_model_names
    )
    assert (
        tuple(
            compile_sql_test_mock_source_names(test_input)
            for test_input in compile_inputs.test_inputs
        )
        == test_case.expected_test_mock_source_names
    )
    assert (
        tuple(
            compile_sql_test_mock_seed_names(test_input)
            for test_input in compile_inputs.test_inputs
        )
        == test_case.expected_test_mock_seed_names
    )
    assert (
        tuple(
            compile_sql_test_expected_model_names(test_input)
            for test_input in compile_inputs.test_inputs
        )
        == test_case.expected_test_expected_model_names
    )
    assert (
        tuple(audit_input.sql_body for audit_input in compile_inputs.audit_inputs)
        == test_case.expected_audit_sql_bodies
    )
    assert (
        tuple(diagnostic.code for diagnostic in compile_inputs.diagnostics)
        == test_case.expected_diagnostic_codes
    )
    assert (
        tuple(
            diagnostic.message.replace(str(tmp_path), "<project>")
            for diagnostic in compile_inputs.diagnostics
        )
        == test_case.expected_diagnostic_messages
    )
    assert (
        tuple(function_input.name for function_input in compile_inputs.sql_function_inputs)
        == test_case.expected_sql_function_names
    )
    actual_function_arguments: list[tuple[tuple[str, str], ...]] = []
    for function_input in compile_inputs.sql_function_inputs:
        arguments: list[tuple[str, str]] = []
        for argument in function_input.arguments:
            arguments.append((argument.name, argument.type))
        actual_function_arguments.append(tuple(arguments))
    assert tuple(actual_function_arguments) == test_case.expected_sql_function_arguments
    assert (
        tuple(function_input.returns for function_input in compile_inputs.sql_function_inputs)
        == test_case.expected_sql_function_returns
    )
    actual_return_columns: list[tuple[tuple[str, str], ...]] = []
    for function_input in compile_inputs.sql_function_inputs:
        return_columns: list[tuple[str, str]] = []
        for column in function_input.return_columns:
            return_columns.append((column.name, column.type))
        actual_return_columns.append(tuple(return_columns))
    assert tuple(actual_return_columns) == test_case.expected_sql_function_return_columns
    assert (
        tuple(function_input.body_sql for function_input in compile_inputs.sql_function_inputs)
        == test_case.expected_sql_function_body_sqls
    )
    assert (
        tuple(function_input.database for function_input in compile_inputs.sql_function_inputs)
        == test_case.expected_sql_function_databases
    )
    assert (
        tuple(function_input.schema for function_input in compile_inputs.sql_function_inputs)
        == test_case.expected_sql_function_schemas
    )
    actual_logical_databases: tuple[str | None, ...] = tuple(
        function_input.logical_database for function_input in compile_inputs.sql_function_inputs
    )
    assert actual_logical_databases == expected_or_actual(
        test_case.expected_sql_function_logical_databases,
        actual_logical_databases,
    )
    actual_logical_schemas: tuple[str | None, ...] = tuple(
        function_input.logical_schema for function_input in compile_inputs.sql_function_inputs
    )
    assert actual_logical_schemas == expected_or_actual(
        test_case.expected_sql_function_logical_schemas,
        actual_logical_schemas,
    )
    assert (
        tuple(str(function_input.language) for function_input in compile_inputs.sql_function_inputs)
        == test_case.expected_sql_function_languages
    )
    assert (
        tuple(
            function_input.runtime_version for function_input in compile_inputs.sql_function_inputs
        )
        == test_case.expected_sql_function_runtime_versions
    )
    assert (
        tuple(function_input.entry_point for function_input in compile_inputs.sql_function_inputs)
        == test_case.expected_sql_function_entry_points
    )
    assert (
        tuple(function_input.packages for function_input in compile_inputs.sql_function_inputs)
        == test_case.expected_sql_function_packages
    )
    actual_model_references: list[tuple[tuple[str, str], ...]] = []
    for model_input in compile_inputs.model_inputs:
        references: list[tuple[str, str]] = []
        for reference in model_input.references:
            references.append((reference.ref_kind, reference.ref_name))
        actual_model_references.append(tuple(references))
    assert tuple(actual_model_references) == test_case.expected_model_references
    actual_audit_references: list[tuple[tuple[str, str], ...]] = []
    for audit_input in compile_inputs.audit_inputs:
        references = []
        for reference in audit_input.references:
            references.append((reference.ref_kind, reference.ref_name))
        actual_audit_references.append(tuple(references))
    assert tuple(actual_audit_references) == test_case.expected_audit_references
    assert compile_inputs.effective_target_name == test_case.expected_effective_target_name
    assert compile_inputs.effective_connection == test_case.expected_effective_connection
    assert (
        compile_inputs.effective_settings.sql_analysis is test_case.expected_effective_sql_analysis
    )
    assert (
        compile_inputs.effective_settings.sql_validation
        is test_case.expected_effective_sql_validation
    )
    assert (
        compile_inputs.effective_settings.concurrency
        == test_case.expected_effective_max_concurrency
    )
    assert compile_inputs.effective_vars == test_case.expected_effective_vars
    assert test_case.run_id is None or compile_inputs.run_id == test_case.run_id
    assert test_case.run_id is not None or re.fullmatch(
        r"\d{8}T\d{6}Z_[0-9a-f]{12}", compile_inputs.run_id
    )


@pytest.mark.parametrize(
    "test_case",
    (
        PreservedFunctionNamespaceTestCase(
            description="retains authored function namespaces while physical targets override",
            expected_defaulted_namespace=(
                "logical_db",
                "function_schema",
                "logical_db",
                "function_schema",
            ),
            expected_explicit_namespace=(
                "function_db",
                "function_schema",
                "function_db",
                "function_schema",
            ),
            expected_unqualified_namespace=(None, None, None, None),
            expected_unqualified_fingerprint_namespace=("logical_db", "function_schema"),
            expected_physical_explicit_namespace=("physical_db", "physical_schema"),
            expected_physical_unqualified_namespace=(None, None),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_preserved_target_when_building_functions_then_logical_namespaces_are_retained(
    test_case: PreservedFunctionNamespaceTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        base_repo_files()
        | {
            "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[defaults]
database = "logical_db"
schema = "logical_schema"
function_schema = "function_schema"

[targets.dev]
database = "preserve"
schema = "preserve"

[targets.physical]
database = "physical_db"
schema = "physical_schema"
""".strip()
            + "\n",
            "functions/sql/defaulted.sql": (
                "FUNCTION (description 'Test function defaulted.', returns INTEGER, tags [reference, daily]);\n\n1\n"
            ),
            "models/model_defaulted.sql": "MODEL (description 'Test model model_defaulted.');\n\nSELECT 1 AS id\n",
            "functions/python/explicit.py": """
from sqlbuild.functions import udf

@udf(
    arguments={},
    returns="INTEGER",
    runtime_version="3.11",
    database="function_db",
    schema="function_schema",
)
def main():
    '''Test function main.'''
    return 1
""".strip()
            + "\n",
            "functions/python/unqualified.py": """
from sqlbuild.functions import udf

@udf(arguments={}, returns="INTEGER", runtime_version="3.11")
def main():
    '''Test function main.'''
    return 1
""".strip()
            + "\n",
        },
    )
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)

    compile_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered_inputs,
        adapter_context=replace(
            DUCKDB_COMPILE_ADAPTER_CONTEXT,
            python_functions_inherit_default_namespace=False,
        ),
    )

    functions: dict[str, FunctionInput] = {
        entry.name: entry for entry in compile_inputs.sql_function_inputs
    }
    assert functions["defaulted"].tags == ("reference", "daily")
    assert compile_inputs.model_inputs[0].config.values["database"] == "logical_db"
    assert compile_inputs.model_inputs[0].config.values["schema"] == "logical_schema"
    assert (
        functions["defaulted"].database,
        functions["defaulted"].schema,
        functions["defaulted"].logical_database,
        functions["defaulted"].logical_schema,
    ) == test_case.expected_defaulted_namespace
    assert (
        functions["explicit"].database,
        functions["explicit"].schema,
        functions["explicit"].logical_database,
        functions["explicit"].logical_schema,
    ) == test_case.expected_explicit_namespace
    assert (
        functions["unqualified"].database,
        functions["unqualified"].schema,
        functions["unqualified"].logical_database,
        functions["unqualified"].logical_schema,
    ) == test_case.expected_unqualified_namespace
    assert (
        functions["unqualified"].fingerprint_database,
        functions["unqualified"].fingerprint_schema,
    ) == test_case.expected_unqualified_fingerprint_namespace

    physical_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered_inputs,
        adapter_context=replace(
            DUCKDB_COMPILE_ADAPTER_CONTEXT,
            python_functions_inherit_default_namespace=False,
        ),
        selected_target="physical",
    )
    physical_functions: dict[str, FunctionInput] = {
        entry.name: entry for entry in physical_inputs.sql_function_inputs
    }
    assert (
        physical_functions["explicit"].database,
        physical_functions["explicit"].schema,
    ) == test_case.expected_physical_explicit_namespace
    assert (
        physical_functions["unqualified"].database,
        physical_functions["unqualified"].schema,
    ) == test_case.expected_physical_unqualified_namespace


@pytest.mark.parametrize(
    "test_case",
    [
        ResolveTargetConfigTestCase(
            description="merges local target overrides with nullable clone policy",
            expected_connection={"warehouse": "local_wh", "role": "project_role"},
            expected_vars={"shared": "local", "project_only": "present"},
            expected_database="local_db",
            expected_schema="project_schema",
            expected_allow_as_clone_origin=False,
            expected_allow_as_clone_destination=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_project_and_local_environment_when_resolving_then_local_values_override_by_field(
    test_case: ResolveTargetConfigTestCase,
) -> None:
    environment: TargetConfig = resolve_target_config(
        project_config=ProjectConfig(
            name="demo",
            adapter="duckdb",
            targets={
                "dev": TargetConfig(
                    connection={"warehouse": "project_wh", "role": "project_role"},
                    vars={"shared": "project", "project_only": "present"},
                    database="project_db",
                    schema="project_schema",
                    clone=ClonePolicy(
                        allow_as_clone_origin=True,
                        allow_as_clone_destination=True,
                    ),
                ),
                "prod": TargetConfig(),
            },
        ),
        local_config=LocalConfig(
            targets={
                "dev": LocalTargetConfig(
                    connection={"warehouse": "local_wh"},
                    vars={"shared": "local"},
                    database="local_db",
                    clone=LocalClonePolicy(allow_as_clone_origin=False),
                )
            }
        ),
        target_name="dev",
    )

    assert environment.connection == test_case.expected_connection
    assert environment.vars == test_case.expected_vars
    assert environment.database == test_case.expected_database
    assert environment.schema == test_case.expected_schema
    assert environment.clone.allow_as_clone_origin is test_case.expected_allow_as_clone_origin
    assert (
        environment.clone.allow_as_clone_destination
        is test_case.expected_allow_as_clone_destination
    )


@pytest.mark.parametrize(
    "test_case",
    [
        BuildCompileInputsDiagnosticTestCase(
            description="reports a table function call without its argument list",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": (
                    'name = "demo"\nadapter = "duckdb"\n\n[settings]\nsql_validation = false\n'
                ),
                "models/orders.sql": (
                    'MODEL (description "Test model orders.");\n\n'
                    'SELECT * FROM __table_fn("customer_orders")\n'
                ),
                "functions/sql/customer_orders.sql": (
                    'FUNCTION (description "Test function customer_orders.",\n'
                    "  arguments (customer_id INTEGER),\n"
                    "  returns table (order_id INTEGER)\n"
                    ");\n\n"
                    "SELECT customer_id AS order_id\n"
                ),
            },
            expected_diagnostics=(
                ("P012", "__table_fn must be followed by an argument list", 3, 15),
            ),
        ),
        BuildCompileInputsDiagnosticTestCase(
            description="reports a table function name that is not double quoted",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": (
                    'name = "demo"\nadapter = "duckdb"\n\n[settings]\nsql_validation = false\n'
                ),
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.');\n\n"
                    "SELECT * FROM __table_fn('customer_orders')(1)\n"
                ),
            },
            expected_diagnostics=(
                (
                    "P012",
                    "__table_fn('customer_orders') is not a valid __table_fn() call",
                    3,
                    15,
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rejected_reference_calls_when_building_compile_inputs_then_reports_located_diagnostics(
    test_case: BuildCompileInputsDiagnosticTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    compile_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
    )

    assert (
        tuple(
            (diagnostic.code, diagnostic.message, diagnostic.line, diagnostic.column)
            for diagnostic in compile_inputs.diagnostics
        )
        == test_case.expected_diagnostics
    )


@pytest.mark.parametrize(
    "test_case",
    [
        BuildCompileInputsErrorTestCase(
            description="raises when a model references an unknown table function",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[settings]
sql_validation = false
""".strip()
                + "\n",
                "models/orders.sql": """
MODEL (description "Test model orders.");

SELECT * FROM __table_fn("missing_orders")(1)
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="references unknown table function 'missing_orders'",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when a table function query contradicts its return columns",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
                "functions/sql/customer_orders.sql": """
FUNCTION (description "Test function customer_orders.",
  arguments (customer_id INTEGER),
  returns table (order_id INTEGER, customer_id INTEGER)
);

SELECT customer_id AS order_id
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="declares 2 return columns but its query produces 1",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when a model calls a scalar function as a table function",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
                "models/orders.sql": (
                    'MODEL (description "Test model orders.");\n\nSELECT * FROM __table_fn("order_total")(1)\n'
                ),
                "functions/sql/order_total.sql": """
FUNCTION (description "Test function order_total.",
  arguments (order_id INTEGER),
  returns INTEGER
);

order_id
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="references scalar function 'order_total' with __table_fn(...)",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when table function argument count does not match",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": (
                    'name = "demo"\nadapter = "duckdb"\n\n[settings]\nsql_validation = false\n'
                ),
                "models/orders.sql": (
                    'MODEL (description "Test model orders.");\n\nSELECT * FROM __table_fn("customer_orders")(1, 2)\n'
                ),
                "functions/sql/customer_orders.sql": """
FUNCTION (description "Test function customer_orders.",
  arguments (customer_id INTEGER),
  returns table (order_id INTEGER)
);

SELECT customer_id AS order_id
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="expects 1 argument but received 2",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when a table function call contains an empty argument",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": (
                    'name = "demo"\nadapter = "duckdb"\n\n[settings]\nsql_validation = false\n'
                ),
                "models/orders.sql": (
                    'MODEL (description "Test model orders.");\n\nSELECT * FROM __table_fn("customer_orders")(1,)\n'
                ),
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="contains an empty argument",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when a table function SQL test argument count does not match",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
                "functions/sql/customer_orders.sql": """
FUNCTION (description "Test function customer_orders.",
  arguments (customer_id INTEGER),
  returns table (order_id INTEGER)
);

SELECT customer_id AS order_id
""".strip()
                + "\n",
                "tests/unit/test_customer_orders.sql": """
TEST (mode table_fn, name "customer_orders_arity");

WITH
__table_fn_actual__ AS (
  SELECT * FROM __table_fn("customer_orders")(1, 2)
),
__table_fn_expected__ AS (SELECT 1 AS order_id)
SELECT 1
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="expects 1 argument but received 2",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when a model references an unknown source",
            repo_files=base_repo_files()
            | {
                "models/staging/orders.sql": """
MODEL (description "Test model orders.");

SELECT * FROM __source("missing_source")
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="references unknown source 'missing_source'",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when authored enforced contract omits incremental cursor",
            repo_files=base_repo_files()
            | {
                "models/orders.sql": """
MODEL (description "Test model orders.",
  materialized incremental,
  contract enforced,
  columns (
    id (type INTEGER),
  ),
  incremental_strategy delete_insert,
  cursor event_time,
  cursor_type timestamp,
  cursor_grain second,
);

SELECT 1 AS id, CURRENT_TIMESTAMP AS event_time
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="cursor references column 'event_time' not declared",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when authored enforced contract omits snapshot check column",
            repo_files=base_repo_files()
            | {
                "models/customer_snapshot.sql": """
MODEL (description "Test model customer_snapshot.",
  materialized snapshot,
  contract enforced,
  columns (
    customer_id (type INTEGER),
    plan (type VARCHAR),
  ),
  unique_key [customer_id],
  snapshot_strategy check,
  check_columns [plan, status],
);

SELECT 1 AS customer_id, 'pro' AS plan, 'active' AS status
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="check_columns references column 'status' not declared",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when authored enforced snapshot appends new columns",
            repo_files=base_repo_files()
            | {
                "models/customer_snapshot.sql": """
MODEL (description "Test model customer_snapshot.",
  materialized snapshot,
  contract enforced,
  columns (
    customer_id (type INTEGER),
    updated_at (type TIMESTAMP),
  ),
  unique_key [customer_id],
  snapshot_strategy timestamp,
  updated_at updated_at,
  snapshot_schema_change append_new_columns,
);

SELECT 1 AS customer_id, CURRENT_TIMESTAMP AS updated_at
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="snapshot_schema_change=append_new_columns is not valid",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when an audit references an unknown source",
            repo_files=base_repo_files()
            | {
                "audits/singular/orders.sql": """
AUDIT ();

SELECT * FROM __source("missing_source")
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="references unknown source 'missing_source'",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when a model uses dbt refs",
            repo_files=base_repo_files()
            | {
                "models/staging/orders.sql": """
MODEL (description "Test model orders.");

SELECT * FROM __dbt_ref("stg_orders")
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment=(
                r'Model file models/staging/orders\.sql uses __dbt_ref\("stg_orders"\) '
                "but no dbt manifest was found"
            ),
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when a SQL function uses dbt refs",
            repo_files=base_repo_files()
            | {
                "functions/sql/orders.sql": """
FUNCTION (description "Test function orders.",
  returns table (order_id INTEGER)
);

SELECT * FROM __dbt_ref("stg_orders")
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment=(
                r'SQL function file functions/sql/orders\.sql uses __dbt_ref\("stg_orders"\) '
                "but dbt refs are not supported yet"
            ),
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when an audit uses dbt refs",
            repo_files=base_repo_files()
            | {
                "audits/singular/orders.sql": """
AUDIT ();

SELECT * FROM __dbt_ref("stg_orders")
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment=(
                r'Audit file audits/singular/orders\.sql may not use __dbt_ref\("stg_orders"\); '
                "audit dbt model checks belong in dbt"
            ),
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when an attached source audit overrides implicit source context",
            repo_files=base_repo_files()
            | {
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    audits:
      - source_not_null:
          source: other_source
""".strip()
                + "\n",
                "audits/generic/source_not_null.sql": """
AUDIT ();

SELECT 1 FROM __source("@source")
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="must not override implicit source from attached context",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when model column allows nulls and uses not null audit",
            repo_files=base_repo_files()
            | {
                "models/staging/orders.sql": """
MODEL (description "Test model orders.",
  columns (
    order_id (nullable true, audits [not_null]),
  ),
);

SELECT 1 AS order_id
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment=(
                "column 'order_id' cannot set nullable = true and audit not_null"
            ),
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when an attached source audit references an unknown generic definition",
            repo_files=base_repo_files()
            | {
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    audits:
      - missing_definition
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="references unknown generic audit 'missing_definition'",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when a compiled test body references an unknown macro",
            repo_files=base_repo_files()
            | {
                "tests/unit/orders_test.sql": """
TEST ();

SELECT @py.missing_macro()
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="Unknown macro '@py.missing_macro'",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when a compiled test body lacks top level test ctes",
            repo_files=base_repo_files()
            | {
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
                "tests/unit/orders_test.sql": """
TEST ();

SELECT 1
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="must declare mock CTEs and one __expected__<model>",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when a compiled test body lacks ceremonial select one",
            repo_files=base_repo_files()
            | {
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
""".strip()
                + "\n",
                "tests/unit/orders_test.sql": """
TEST ();

WITH
__source__raw_orders AS (
  SELECT 1 AS order_id
),
__expected__orders AS (
  SELECT 1 AS order_id
)
SELECT order_id FROM __expected__orders
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="must end after its CTEs",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when a compiled test body references an unknown source mock",
            repo_files=base_repo_files()
            | {
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
                "tests/unit/orders_test.sql": """
TEST ();

WITH
__source__missing_source AS (
  SELECT 1 AS order_id
),
__expected__orders AS (
  SELECT 1 AS order_id
)
SELECT 1
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="mocks unknown source 'missing_source'",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when a compiled test body references an unknown seed mock",
            repo_files=base_repo_files()
            | {
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
                "tests/unit/orders_test.sql": """
TEST ();

WITH
__seed__missing_seed AS (
  SELECT 1 AS id
),
__expected__orders AS (
  SELECT 1 AS id
)
SELECT 1
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="mocks unknown seed 'missing_seed'",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when dbt source fixture collides through compile attachment",
            repo_files=base_repo_files()
            | {
                "dbt/target/manifest.json": """
{
  "nodes": {},
  "sources": {
    "source.analytics.raw.orders": {
      "unique_id": "source.analytics.raw.orders",
      "resource_type": "source",
      "package_name": "analytics",
      "source_name": "raw",
      "name": "orders",
      "relation_name": "raw.orders"
    }
  },
  "macros": {}
}
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
                "sources/raw.yml": """
sources:
  - name: raw__orders
    description: Test source raw__orders.
""".strip()
                + "\n",
                "tests/unit/orders_test.sql": """
TEST ();

WITH
__source__raw__orders AS (
  SELECT 1 AS order_id
),
__expected__orders AS (
  SELECT 1 AS order_id
)
SELECT 1
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="conflicts with a SQLBuild source",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when dbt seed fixture collides through compile attachment",
            repo_files=base_repo_files()
            | {
                "dbt/target/manifest.json": """
{
  "nodes": {
    "seed.analytics.countries": {
      "unique_id": "seed.analytics.countries",
      "resource_type": "seed",
      "package_name": "analytics",
      "name": "countries",
      "relation_name": "analytics.countries"
    }
  },
  "sources": {},
  "macros": {}
}
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
                "seeds/countries.csv": "country_code,country_name\nUS,United States\n",
                "seeds/schema.yml": """
seeds:
  - name: countries
    description: Test seed countries.
    columns:
      - name: country_code
        type: VARCHAR
      - name: country_name
        type: VARCHAR
""".strip()
                + "\n",
                "tests/unit/orders_test.sql": """
TEST ();

WITH
__seed__countries AS (
  SELECT 'US' AS country_code
),
__expected__orders AS (
  SELECT 1 AS order_id
)
SELECT 1
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="conflicts with a SQLBuild seed",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when a compiled test body references an unknown macro mock",
            repo_files=base_repo_files()
            | {
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
""".strip()
                + "\n",
                "tests/unit/orders_test.sql": """
TEST ();

WITH
__macro__missing_macro AS (
  SELECT '1'
),
__source__raw_orders AS (
  SELECT 1 AS order_id
),
__expected__orders AS (
  SELECT 1 AS order_id
)
SELECT 1
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="mocks unknown macro 'missing_macro'",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when a compiled test body references an unknown expected model",
            repo_files=base_repo_files()
            | {
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
""".strip()
                + "\n",
                "tests/unit/orders_test.sql": """
TEST ();

WITH
__source__raw_orders AS (
  SELECT 1 AS order_id
),
__expected__missing_model AS (
  SELECT 1 AS order_id
)
SELECT 1
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="expects unknown model 'missing_model'",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when a compiled test body uses a reserved helper cte name",
            repo_files=base_repo_files()
            | {
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
""".strip()
                + "\n",
                "tests/unit/orders_test.sql": """
TEST ();

WITH
__actual AS (
  SELECT 1 AS order_id
),
__source__raw_orders AS (
  SELECT * FROM __actual
),
__expected__orders AS (
  SELECT 1 AS order_id
)
SELECT 1
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="uses reserved helper CTE name '__actual'",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when a compiled audit body references an unknown macro",
            repo_files=base_repo_files()
            | {
                "audits/singular/orders.sql": """
AUDIT ();

SELECT @py.missing_macro()
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="Unknown macro '@py.missing_macro'",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when macro files collide during compile",
            repo_files=base_repo_files()
            | {
                "macros/python/common.py": """
def project_columns() -> str:
    return "order_id"
""".strip()
                + "\n",
                "macros/python/nested/common.py": """
def project_columns() -> str:
    return "customer_id"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="Macro name collision for 'project_columns'",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when a macro identity is noncanonical",
            repo_files=base_repo_files()
            | {
                "macros/python/orders.py": "def DailyOrders():\n    return 'order_id'\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect @py.DailyOrders()\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment=(
                "Invalid macro identity 'DailyOrders'.*use snake_case 'daily_orders'"
            ),
            expected_error_type=ResourceIdentityError,
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when model config field contains a macro call",
            repo_files=base_repo_files()
            | {
                "macros/python/common.py": """
def dynamic_schema() -> str:
    return "marts"
""".strip()
                + "\n",
                "models/staging/orders.sql": """
MODEL (description "Test model orders.",
  schema '@py.dynamic_schema()',
);

select 1
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="model config field 'schema' does not allow macros",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when the selected target does not exist",
            repo_files=base_repo_files()
            | {
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n"
            },
            selected_target="missing",
            run_id=None,
            expected_error_fragment="Unknown target 'missing'",
            expected_error_type=SpecConfigError,
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when the local target does not exist",
            repo_files=base_repo_files()
            | {
                "sqlbuild_local.toml": """
target = "missing"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="Unknown target 'missing'",
            expected_error_type=SpecConfigError,
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when the project default target does not exist",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "missing"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="Unknown target 'missing'",
            expected_error_type=SpecConfigError,
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when effective vars reference an unknown project variable",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[vars]
user = "${missing}"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="effective vars references unknown variable 'missing'",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when connection templates reference missing env vars",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[connection]
path = "${ENV:SQLBUILD_DB_PATH}"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment=(
                "effective connection references missing ENV variable 'SQLBUILD_DB_PATH'"
            ),
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when model config uses unknown ctx keys",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
""".strip()
                + "\n",
                "models/staging/orders.sql": """
MODEL (description "Test model orders.",
  schema '${CTX:this}',
);

select 1
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="model config references unknown CTX key 'this'",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when environment override references an unknown ctx key",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[defaults]
schema = "marts"

[targets]

[targets.dev]
schema = "dev_${CTX:destination.missing}"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment=(
                "environment schema references unknown CTX key 'destination.missing'"
            ),
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when environment database override references unavailable ctx value",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[targets]

[targets.dev]
database = "dev_${CTX:model.database}"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment=(
                "environment database references CTX key 'model.database' but no value is available"
            ),
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when effective vars contain a cyclic reference",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[vars]
first = "${second}"
second = "${first}"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment=(
                "effective vars contain a cyclic reference: first -> second -> first"
            ),
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when templates use an unsupported namespace",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[vars]
user = "kevin"

[defaults]
schema = "${SQLBUILD:user}"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="model config references unsupported template namespace 'SQLBUILD'",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when connection values use disallowed ctx templates",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[connection]
path = "${CTX:schema}"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="effective connection does not allow CTX templates",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when plural hooks use bare SQL strings",
            repo_files=base_repo_files()
            | {
                "models/staging/broken.sql": (
                    "MODEL (description 'Test model broken.', post_hooks ['SELECT 1']);\n\nSELECT 1 AS id\n"
                ),
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="post_hooks entries must use typed inline_sql",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when inline hook renders an empty payload",
            repo_files=base_repo_files()
            | {
                "models/staging/broken.sql": (
                    "MODEL (description 'Test model broken.', post_hooks [inline_sql('')]);\n\nSELECT 1 AS id\n"
                ),
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment=(
                r"post_hooks\[0\] inline_sql\(\"\.\.\.\"\).*must render a non-empty SQL payload"
            ),
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when named hook arguments render an empty payload",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": (
                    'name = "demo"\nadapter = "duckdb"\n\n[settings]\nsql_validation = false\n'
                ),
                "models/staging/broken.sql": (
                    'MODEL (description "Test model broken.", post_hooks [sql("dynamic", statement: "")]);\n\nSELECT 1 AS id\n'
                ),
                "hooks/sql/dynamic.sql": "HOOK (description 'Test hook dynamic.');\n\n@statement\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment=(
                r"post_hooks\[0\] sql\(\"dynamic\"\).*must render a non-empty SQL payload"
            ),
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when hook sql uses config template syntax",
            repo_files=base_repo_files()
            | {
                "models/staging/broken.sql": (
                    "MODEL (description 'Test model broken.', post_hooks ["
                    "inline_sql('GRANT SELECT ON ${CTX:destination.qualified} TO analyst')"
                    "]);\n\n"
                    "SELECT 1 AS id\n"
                ),
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment=(
                r"post_hooks\[0\] inline_sql\(\"\.\.\.\"\) .* uses unsupported \$\{\.\.\.\} "
                r"template syntax"
            ),
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when python hook references unknown hook name",
            repo_files=base_repo_files()
            | {
                "models/staging/broken.sql": (
                    'MODEL (description "Test model broken.", post_hooks [python("notify")]);\n\nSELECT 1 AS id\n'
                ),
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment=(
                r"post_hooks\[0\] python\(\"notify\"\) references an unknown hook"
            ),
        ),
        BuildCompileInputsErrorTestCase(
            description="raises with nonzero hook index when later python hook is invalid",
            repo_files=base_repo_files()
            | {
                "models/staging/broken.sql": (
                    'MODEL (description "Test model broken.", post_hooks [python("known"), python("missing")]);\n\nSELECT 1 AS id\n'
                ),
                "hooks/python/notifications.py": """
from sqlbuild.hooks import hook


@hook
def known(ctx):
    '''Test hook known.'''
    return None
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment=(
                r"post_hooks\[1\] python\(\"missing\"\) references an unknown hook"
            ),
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when python hook is missing required kwarg",
            repo_files=base_repo_files()
            | {
                "models/staging/broken.sql": (
                    'MODEL (description "Test model broken.", pre_hooks [python("notify")]);\n\nSELECT 1 AS id\n'
                ),
                "hooks/python/notifications.py": """
from sqlbuild.hooks import hook


@hook
def notify(ctx, channel):
    '''Test hook notify.'''
    return None
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment=(
                r"pre_hooks\[0\] python\(\"notify\"\) is missing required argument\(s\): channel"
            ),
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when python hook receives unknown kwarg",
            repo_files=base_repo_files()
            | {
                "models/staging/broken.sql": (
                    'MODEL (description "Test model broken.", post_hooks [python("notify", unknown: "value")]);\n\nSELECT 1 AS id\n'
                ),
                "hooks/python/notifications.py": """
from sqlbuild.hooks import hook


@hook
def notify(ctx):
    '''Test hook notify.'''
    return None
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment=(
                r"post_hooks\[0\] python\(\"notify\"\) has unknown argument\(s\): unknown"
            ),
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when python hook requires positional only argument",
            repo_files=base_repo_files()
            | {
                "models/staging/broken.sql": (
                    'MODEL (description "Test model broken.", post_hooks [python("notify")]);\n\nSELECT 1 AS id\n'
                ),
                "hooks/python/notifications.py": """
from sqlbuild.hooks import hook


@hook
def notify(ctx, channel, /):
    '''Test hook notify.'''
    return None
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment=(
                r"post_hooks\[0\] python\(\"notify\"\) cannot be configured .* "
                r"channel"
            ),
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when model header tags is a string instead of list",
            repo_files=base_repo_files()
            | {
                "models/staging/orders.sql": "MODEL (description 'Test model orders.', tags nightly);\n\nSELECT 1\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="tags must be a list",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when dbt ref has no discovered manifest",
            repo_files=base_repo_files()
            | {
                "models/fact_orders.sql": 'MODEL (description "Test model fact_orders.");\n\nselect * from __dbt_ref("orders")\n',
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="but no dbt manifest was found",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when dbt ref is ambiguous in discovered manifest",
            repo_files=base_repo_files()
            | {
                "dbt/target/manifest.json": """
{
  "nodes": {
    "model.analytics.orders": {
      "unique_id": "model.analytics.orders",
      "resource_type": "model",
      "package_name": "analytics",
      "name": "orders",
      "relation_name": "analytics.orders"
    },
    "model.stripe.orders": {
      "unique_id": "model.stripe.orders",
      "resource_type": "model",
      "package_name": "stripe",
      "name": "orders",
      "relation_name": "stripe.orders"
    }
  }
}
""".strip()
                + "\n",
                "models/fact_orders.sql": 'MODEL (description "Test model fact_orders.");\n\nselect * from __dbt_ref("orders")\n',
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="dbt model 'orders' is ambiguous across packages",
        ),
        BuildCompileInputsErrorTestCase(
            description="raises when dbt and SQLBuild model names overlap",
            repo_files=base_repo_files()
            | {
                "dbt/target/manifest.json": """
{
  "nodes": {
    "model.analytics.orders": {
      "unique_id": "model.analytics.orders",
      "resource_type": "model",
      "package_name": "analytics",
      "name": "orders",
      "relation_name": "analytics.orders"
    }
  }
}
""".strip()
                + "\n",
                "models/orders.sql": "MODEL (description 'Test model orders.');\n\nselect 1\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="dbt and SQLBuild models share names: orders",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_attachment_conflicts_when_building_compile_inputs_then_it_raises_clear_errors(
    test_case: BuildCompileInputsErrorTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    target_name: str
    environment_value: str
    for target_name, environment_value in test_case.environment_variables.items():
        monkeypatch.setenv(target_name, environment_value)

    write_repo_files(tmp_path, test_case.repo_files)

    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)

    with pytest.raises(test_case.expected_error_type, match=test_case.expected_error_fragment):
        build_compile_inputs(
            discovered_inputs=discovered_inputs,
            adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
            selected_target=test_case.selected_target,
            run_id=test_case.run_id,
            external_sql_reference_resolver=build_external_sql_reference_resolver(
                project_dir=tmp_path
            ),
        )


@pytest.mark.parametrize(
    "test_case",
    [
        BuildCompileInputsPythonHookValidationTestCase(
            description="Python hook validation inspects signatures without executing hooks",
            repo_files=base_repo_files()
            | {
                "models/staging/orders.sql": (
                    'MODEL (description "Test model orders.", post_hooks [python("notify", channel: "alerts")]);\n\nSELECT 1 AS id\n'
                ),
                "hooks/python/notifications.py": """
from pathlib import Path
from sqlbuild.hooks import hook


@hook
def notify(ctx, channel):
    '''Test hook notify.'''
    Path(__file__).with_name("executed.marker").write_text("executed", encoding="utf-8")
""".strip()
                + "\n",
            },
            expected_marker_file_exists=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_hook_when_building_compile_inputs_then_validation_does_not_execute_hook(
    test_case: BuildCompileInputsPythonHookValidationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)

    build_compile_inputs(
        discovered_inputs=discovered_inputs,
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
    )

    assert (
        tmp_path / "hooks" / "executed.marker"
    ).is_file() is test_case.expected_marker_file_exists


@pytest.mark.parametrize(
    "test_case",
    [
        BuildModelConfigHooksTestCase(
            description="layers typed hooks from defaults and path defaults",
            default_hook=[
                SqlHookEntry(statement="CREATE TABLE audit_log AS SELECT 'default' AS source")
            ],
            path_default_hook=[
                SqlHookEntry(statement="INSERT INTO audit_log SELECT 'path' AS source")
            ],
            expected_pre_hooks=[
                SqlHookEntry(statement="CREATE TABLE audit_log AS SELECT 'default' AS source")
            ],
            expected_post_hooks=[
                SqlHookEntry(statement="INSERT INTO audit_log SELECT 'path' AS source")
            ],
        )
    ],
    ids=lambda case: case.description,
)
def test_given_typed_hook_defaults_when_building_config_then_layers_values(
    test_case: BuildModelConfigHooksTestCase,
) -> None:
    defaults: DefaultsConfig = DefaultsConfig(pre_hooks=test_case.default_hook)
    path_defaults: dict[str, dict[str, object]] = {
        "models/marts": {"post_hooks": test_case.path_default_hook}
    }

    config: CompileModelConfig = build_config_natively(
        defaults=defaults,
        path_defaults=path_defaults,
        matched_path_default="models/marts",
        model_header_values={},
        target_config=None,
    )

    assert config.values["pre_hooks"] == test_case.expected_pre_hooks
    assert config.values["post_hooks"] == test_case.expected_post_hooks


@pytest.mark.parametrize(
    "test_case",
    [
        BuildModelRetentionConfigTestCase(
            description="model duration overrides materialization and target",
            defaults=DefaultsConfig(materialized="table"),
            model_header_values={"time_travel_retention": "30d"},
            target_config=TargetConfig(
                time_travel_retention=AuthoredTimeTravelRetention(desired_days=7)
            ),
            materialization_defaults=MaterializationDefaultsConfig(
                table=MaterializationRetentionDefaults(
                    time_travel_retention=AuthoredTimeTravelRetention(desired_days=14)
                )
            ),
            expected_desired_days=30,
            expected_unmanaged=False,
            expected_source=TimeTravelRetentionSource.MODEL,
        ),
        BuildModelRetentionConfigTestCase(
            description="zero-day model retention remains valid",
            defaults=DefaultsConfig(materialized="table"),
            model_header_values={"time_travel_retention": "0d"},
            target_config=None,
            materialization_defaults=MaterializationDefaultsConfig(),
            expected_desired_days=0,
            expected_unmanaged=False,
            expected_source=TimeTravelRetentionSource.MODEL,
        ),
        BuildModelRetentionConfigTestCase(
            description="model disabled overrides inherited view policy",
            defaults=DefaultsConfig(materialized="view"),
            model_header_values={"time_travel_retention": "disabled"},
            target_config=None,
            materialization_defaults=MaterializationDefaultsConfig(),
            expected_desired_days=None,
            expected_unmanaged=True,
            expected_source=TimeTravelRetentionSource.MODEL,
        ),
        BuildModelRetentionConfigTestCase(
            description="incremental model inherits target retention",
            defaults=DefaultsConfig(materialized="incremental"),
            model_header_values={},
            target_config=TargetConfig(
                time_travel_retention=AuthoredTimeTravelRetention(desired_days=90)
            ),
            materialization_defaults=MaterializationDefaultsConfig(),
            expected_desired_days=90,
            expected_unmanaged=False,
            expected_source=TimeTravelRetentionSource.TARGET,
        ),
        BuildModelRetentionConfigTestCase(
            description="view does not inherit target retention",
            defaults=DefaultsConfig(materialized="view"),
            model_header_values={},
            target_config=TargetConfig(
                time_travel_retention=AuthoredTimeTravelRetention(desired_days=90)
            ),
            materialization_defaults=MaterializationDefaultsConfig(),
            expected_desired_days=None,
            expected_unmanaged=True,
            expected_source=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_retention_layers_when_building_model_config_then_precedence_is_resolved(
    test_case: BuildModelRetentionConfigTestCase,
) -> None:
    config: CompileModelConfig = build_config_natively(
        defaults=test_case.defaults,
        path_defaults={},
        matched_path_default=None,
        model_header_values=test_case.model_header_values,
        target_config=test_case.target_config,
        materialization_defaults=test_case.materialization_defaults,
        effective_target_name="prod",
    )

    assert config.time_travel_retention.desired_days == test_case.expected_desired_days
    assert config.time_travel_retention.unmanaged is test_case.expected_unmanaged
    assert config.time_travel_retention.source is test_case.expected_source
    assert "time_travel_retention" not in config.values


@pytest.mark.parametrize(
    "test_case",
    [
        AuthoredRetentionCompileTestCase(
            description="model header overrides project retention layers",
            repo_files={
                "sqlbuild_project.toml": "\n".join(
                    (
                        'name = "demo"',
                        'adapter = "duckdb"',
                        'default_target = "prod"',
                        "[targets.prod]",
                        'time_travel_retention = "7d"',
                        "[materialization_defaults.table]",
                        'time_travel_retention = "14d"',
                    )
                ),
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.', materialized table, time_travel_retention 30d);\n\nSELECT 1 AS id\n"
                ),
            },
            expected_desired_days=30,
            expected_source=TimeTravelRetentionSource.MODEL,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_authored_retention_header_when_building_inputs_then_typed_policy_is_attached(
    test_case: AuthoredRetentionCompileTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)

    compile_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered_inputs,
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
    )

    policy: ResolvedTimeTravelRetention = compile_inputs.model_inputs[
        0
    ].config.time_travel_retention
    assert policy.desired_days == test_case.expected_desired_days
    assert policy.source is test_case.expected_source


@pytest.mark.parametrize(
    "test_case",
    [
        BuildCompileInputsErrorTestCase(
            description="snapshot append new columns contract conflict has code",
            repo_files=base_repo_files()
            | {
                "models/customer_snapshot.sql": """
MODEL (description "Test model customer_snapshot.",
  materialized snapshot,
  contract enforced,
  columns (
    customer_id (type INTEGER),
    updated_at (type TIMESTAMP),
  ),
  unique_key [customer_id],
  snapshot_strategy timestamp,
  updated_at updated_at,
  snapshot_schema_change append_new_columns,
);

SELECT 1 AS customer_id, CURRENT_TIMESTAMP AS updated_at
""".strip()
                + "\n",
            },
            selected_target=None,
            run_id=None,
            expected_error_fragment="snapshot_schema_change=append_new_columns is not valid",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snapshot_contract_schema_change_conflict_when_building_then_error_has_code(
    test_case: BuildCompileInputsErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)

    with pytest.raises(ValueError, match=test_case.expected_error_fragment) as exc_info:
        build_compile_inputs(
            discovered_inputs=discovered_inputs,
            adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
            selected_target=test_case.selected_target,
            run_id=test_case.run_id,
            external_sql_reference_resolver=build_external_sql_reference_resolver(
                project_dir=tmp_path
            ),
        )

    assert getattr(exc_info.value, "code", None) == "K012"


@pytest.mark.parametrize(
    "test_case",
    [
        SeedRefRegressionTestCase(
            description="model referencing a seed via __seed compiles successfully",
            repo_files=base_repo_files()
            | {
                "seeds/waffle_types.csv": "waffle_type_id,waffle_name\n1,Classic\n",
                "seeds/schema.yml": (
                    "seeds:\n"
                    "  - name: waffle_types\n    description: Test seed waffle_types.\n"
                    "    columns:\n"
                    "      - name: waffle_type_id\n"
                    "        type: INTEGER\n"
                    "      - name: waffle_name\n"
                    "        type: VARCHAR\n"
                ),
                "models/orders.sql": (
                    'MODEL (description "Test model orders.");\n\nSELECT waffle_type_id FROM __seed("waffle_types")'
                ),
            },
            expected_model_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_model_referencing_seed_when_building_compile_inputs_then_succeeds(
    test_case: SeedRefRegressionTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    compile_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered_inputs,
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
    )

    assert len(compile_inputs.model_inputs) == test_case.expected_model_count
