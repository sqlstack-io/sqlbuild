from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from unittest.mock import call, patch

import pytest

from sqlbuild._native import (
    DiscoveredProjectInputs,
    DiscoveredSqlTestBlock,
    ProjectConfigError,
    ProjectPythonPathError,
    discover_project_inputs,
)
from sqlbuild.observability import EventDispatcher, LifecycleEvent, dispatcher_scope
from tests.unit.src.sqlbuild.compiler.discovery._test_helpers import (
    base_repo_files,
)
from tests.unit.src.sqlbuild.compiler.discovery._test_types import (
    DiscoverFactoryValidationTestCase,
    DiscoverProjectInputsErrorTestCase,
    DiscoverProjectInputsTestCase,
    DiscoverUniqueTestNameTestCase,
    DiscoveryLifecycleTestCase,
    DiscoveryRelevantCountTestCase,
    ProjectPythonPathAcceptanceTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverProjectInputsTestCase(
            description="discovers raw project inputs across authored project surfaces",
            repo_files=base_repo_files()
            | {
                "models/staging/orders.sql": "MODEL ();\n\nselect 1\n",
                "models/staging/schema.yml": "models: []\n",
                "sources/raw.yml": """
sources:
  - name: raw_orders
    managed: true
    schema: public
    table: orders
""".strip()
                + "\n",
                "seeds/country_codes.csv": "country_code,country_name\nUS,United States\n",
                "seeds/lookups.yml": """
seeds:
  - name: country_codes
    columns:
      - name: country_code
        type: VARCHAR
      - name: country_name
        type: VARCHAR
""".strip()
                + "\n",
                "tests/unit/orders_test.sql": "TEST ();\nSELECT 1\n",
                "tests/legacy_orders.sql": "TEST ();\nSELECT 2\n",
                "tests/scenarios/revenue/revenue__customer_refund.sql": """
SCENARIO (description "Customer refund", tags ["revenue"]);

WITH
__expected__daily_revenue AS (
  SELECT 1 AS order_id
)
SELECT 1
""",
                "audits/generic/not_null.sql": "AUDIT ();\nSELECT 1\n",
                "macros/python/name_helpers.py": "def slug() -> str:\n    return 'slug'\n",
                "python/loaders/raw_orders.py": """
from sqlbuild.loaders import loader

@loader(name="raw_orders")
def fetch_orders(ctx):
    return []
""",
                "python/tasks/windows.py": """
from sqlbuild.tasks import task

@task(tags=("api",), group="ingestion")
def fetch_window(ctx):
    return {"window": "today"}
""",
                "python/assets/exports.py": """
from sqlbuild.assets import asset

@asset(columns=[{"name": "customer_id", "type": "string"}])
def export_customers(ctx):
    return {"uri": "s3://exports/customers.parquet"}
""",
                "python/checks/exports.py": """
from sqlbuild.checks import check
from python.assets.exports import export_customers

@check(depends_on=export_customers, severity="warn")
def export_customers_exists(ctx):
    return True
""",
                "hooks/python/notifications.py": """
from sqlbuild.hooks import hook

@hook(name="notify_success")
def notify_success(ctx):
    return None
""",
                "providers/slack.py": """
from sqlbuild.providers import Provider

class SlackProvider(Provider):
    channel: str = "#alerts"
""",
                "target/manifest.json": '{"metadata": {"dbt_schema_version": "v12"}}\n',
                "adapter.py": "class ExampleAdapter:\n    pass\n",
                "sqlbuild_local.toml": 'target = "dev"\n',
            },
            expected_model_paths=("models/staging/orders.sql",),
            expected_model_header_values=({},),
            expected_model_query_sql=("select 1",),
            expected_schema_paths=("models/staging/schema.yml", "seeds/lookups.yml"),
            expected_schema_model_names=((), ()),
            expected_schema_seed_names=((), ("country_codes",)),
            expected_source_paths=("sources/raw.yml",),
            expected_source_entry_names=(("raw_orders",),),
            expected_seed_paths=("seeds/country_codes.csv",),
            expected_test_paths=("tests/unit/orders_test.sql",),
            expected_test_block_indexes=(1,),
            expected_test_block_names=(None,),
            expected_test_block_sql_bodies=("SELECT 1",),
            expected_scenario_paths=("tests/scenarios/revenue/revenue__customer_refund.sql",),
            expected_scenario_names=("revenue__customer_refund",),
            expected_scenario_header_values=(
                {"description": "Customer refund", "tags": ["revenue"]},
            ),
            expected_scenario_sql_bodies=(
                "WITH\n__expected__daily_revenue AS (\n  SELECT 1 AS order_id\n)\nSELECT 1",
            ),
            expected_audit_paths=("audits/generic/not_null.sql",),
            expected_audit_block_indexes=(1,),
            expected_audit_block_names=(None,),
            expected_audit_block_sql_bodies=("SELECT 1",),
            expected_macro_paths=("macros/python/name_helpers.py",),
            expected_loader_names=("raw_orders",),
            expected_adapter_path="adapter.py",
            expected_task_names=("fetch_window",),
            expected_asset_names=("export_customers",),
            expected_check_names=("export_customers_exists",),
            expected_hook_names=("notify_success",),
            expected_provider_names=("slack_provider",),
            expected_provider_paths=("providers/slack.py",),
            expected_provider_class_names=("SlackProvider",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_project_repo_slice_when_discovering_inputs_then_it_returns_expected_raw_inventory(
    test_case: DiscoverProjectInputsTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    discovered_inputs: object = discover_project_inputs(project_dir=tmp_path)

    assert (
        tuple(str(model_file.relative_path) for model_file in discovered_inputs.model_files)
        == test_case.expected_model_paths
    )
    assert (
        tuple(model_file.header_values for model_file in discovered_inputs.model_files)
        == test_case.expected_model_header_values
    )
    assert (
        tuple(model_file.query_sql for model_file in discovered_inputs.model_files)
        == test_case.expected_model_query_sql
    )
    assert (
        tuple(str(schema_file.relative_path) for schema_file in discovered_inputs.schema_files)
        == test_case.expected_schema_paths
    )
    actual_schema_model_names: list[tuple[str, ...]] = []
    actual_schema_seed_names: list[tuple[str, ...]] = []
    for schema_file in discovered_inputs.schema_files:
        model_names: list[str] = []
        for model_entry in schema_file.model_entries:
            model_names.append(model_entry.name)
        actual_schema_model_names.append(tuple(model_names))
        seed_names: list[str] = []
        for seed_entry in schema_file.seed_entries:
            seed_names.append(seed_entry.name)
        actual_schema_seed_names.append(tuple(seed_names))
    assert tuple(actual_schema_model_names) == test_case.expected_schema_model_names
    assert tuple(actual_schema_seed_names) == test_case.expected_schema_seed_names
    assert (
        tuple(str(source_file.relative_path) for source_file in discovered_inputs.source_files)
        == test_case.expected_source_paths
    )
    actual_source_entry_names: list[tuple[str, ...]] = []
    for source_file in discovered_inputs.source_files:
        source_names: list[str] = []
        for source_entry in source_file.source_entries:
            source_names.append(source_entry.name)
        actual_source_entry_names.append(tuple(source_names))
    assert tuple(actual_source_entry_names) == test_case.expected_source_entry_names
    assert (
        tuple(str(seed_file.relative_path) for seed_file in discovered_inputs.seed_files)
        == test_case.expected_seed_paths
    )
    assert (
        tuple(str(test_file.relative_path) for test_file in discovered_inputs.test_files)
        == test_case.expected_test_paths
    )
    assert (
        tuple(block.test_index for block in discovered_inputs.test_files[0].blocks)
        == test_case.expected_test_block_indexes
    )
    assert (
        tuple(block.name for block in discovered_inputs.test_files[0].blocks)
        == test_case.expected_test_block_names
    )
    assert (
        tuple(block.sql_body for block in discovered_inputs.test_files[0].blocks)
        == test_case.expected_test_block_sql_bodies
    )
    assert (
        tuple(
            str(scenario_file.relative_path) for scenario_file in discovered_inputs.scenario_files
        )
        == test_case.expected_scenario_paths
    )
    assert (
        tuple(scenario_file.name for scenario_file in discovered_inputs.scenario_files)
        == test_case.expected_scenario_names
    )
    assert (
        tuple(scenario_file.header_values for scenario_file in discovered_inputs.scenario_files)
        == test_case.expected_scenario_header_values
    )
    assert (
        tuple(scenario_file.sql_body for scenario_file in discovered_inputs.scenario_files)
        == test_case.expected_scenario_sql_bodies
    )
    assert (
        tuple(str(audit_file.relative_path) for audit_file in discovered_inputs.audit_files)
        == test_case.expected_audit_paths
    )
    assert (
        tuple(block.audit_index for block in discovered_inputs.audit_files[0].blocks)
        == test_case.expected_audit_block_indexes
    )
    assert (
        tuple(block.name for block in discovered_inputs.audit_files[0].blocks)
        == test_case.expected_audit_block_names
    )
    assert (
        tuple(block.sql_body for block in discovered_inputs.audit_files[0].blocks)
        == test_case.expected_audit_block_sql_bodies
    )
    assert (
        tuple(str(macro_file.relative_path) for macro_file in discovered_inputs.macro_files)
        == test_case.expected_macro_paths
    )
    assert (
        tuple(loader_function.name for loader_function in discovered_inputs.loader_functions)
        == test_case.expected_loader_names
    )
    assert (
        tuple(task_function.name for task_function in discovered_inputs.task_functions)
        == test_case.expected_task_names
    )
    assert (
        tuple(asset_function.name for asset_function in discovered_inputs.asset_functions)
        == test_case.expected_asset_names
    )
    assert (
        tuple(check_function.name for check_function in discovered_inputs.check_functions)
        == test_case.expected_check_names
    )
    assert (
        tuple(hook_function.name for hook_function in discovered_inputs.hook_functions)
        == test_case.expected_hook_names
    )
    assert (
        tuple(provider.name for provider in discovered_inputs.providers)
        == test_case.expected_provider_names
    )
    assert (
        tuple(str(provider.relative_path) for provider in discovered_inputs.providers)
        == test_case.expected_provider_paths
    )
    assert (
        tuple(provider.provider_class.__name__ for provider in discovered_inputs.providers)
        == test_case.expected_provider_class_names
    )
    assert tuple(provider.settings.__class__ for provider in discovered_inputs.providers) == tuple(
        provider.provider_class for provider in discovered_inputs.providers
    )
    adapter_paths: tuple[str | None, ...] = (
        None,
        str(getattr(discovered_inputs.adapter_file, "relative_path", "")),
    )
    assert (
        adapter_paths[discovered_inputs.adapter_file is not None] == test_case.expected_adapter_path
    )
    assert discovered_inputs.project_config.name == "demo"
    assert discovered_inputs.project_config.adapter == "duckdb"
    assert discovered_inputs.local_config.target == "dev"


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoveryLifecycleTestCase(
            description="declarations complete before python failure",
            expected_event_types=(
                "operation_started",
                "operation_started",
                "operation_started",
                "operation_completed",
                "operation_started",
                "operation_failed",
                "operation_failed",
                "operation_failed",
            ),
            expected_operation_names=(
                "project_discovery",
                "discovery_project_assembly",
                "discovery_declaration_parse",
                "discovery_declaration_parse",
                "discovery_python_import",
                "discovery_python_import",
                "discovery_project_assembly",
                "project_discovery",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_valid_sql_and_failing_python_import_when_discovering_then_phases_are_attributed(
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    test_case: DiscoveryLifecycleTestCase,
) -> None:
    write_repo_files(
        tmp_path,
        base_repo_files()
        | {
            "models/orders.sql": "MODEL ();\n\nSELECT 1 AS order_id\n",
            "python/tasks/broken.py": "import dependency_that_does_not_exist\n",
        },
    )
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with dispatcher_scope(dispatcher):
        with pytest.raises(Exception, match="dependency_that_does_not_exist"):
            discover_project_inputs(project_dir=tmp_path)

    assert tuple(event.event_type for event in events) == test_case.expected_event_types
    assert tuple(str(event.payload["operation_name"]) for event in events) == (
        test_case.expected_operation_names
    )


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoveryRelevantCountTestCase(
            description="ignored siblings are not traversed or counted",
            expected_item_count=1,
            unexpected_root_pattern="*",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_ignored_siblings_when_discovering_then_count_uses_relevant_collections_only(
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    test_case: DiscoveryRelevantCountTestCase,
) -> None:
    write_repo_files(
        tmp_path,
        base_repo_files()
        | {
            "models/orders.sql": "MODEL ();\n\nSELECT 1 AS order_id\n",
            ".git/objects/unrelated.sql": "not sqlbuild input\n",
            ".venv/cache/generated.sql": "not sqlbuild input\n",
        },
    )
    events: list[LifecycleEvent] = []
    dispatcher: EventDispatcher = EventDispatcher()
    dispatcher.subscribe_lifecycle(subscriber=events.append, accepts_opaque=False)

    with patch.object(Path, "rglob", autospec=True, side_effect=Path.rglob) as rglob:
        with dispatcher_scope(dispatcher):
            discover_project_inputs(project_dir=tmp_path)

    assert call(tmp_path, test_case.unexpected_root_pattern) not in rglob.call_args_list
    assert events[3].payload["operation_name"] == "discovery_declaration_parse"
    assert events[3].payload["metadata"] == {"item_count": test_case.expected_item_count}


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoverFactoryValidationTestCase(
            description="generated managed source loader matches source",
            repo_files=base_repo_files()
            | {
                "sources/raw.yml": "sources:\n  - name: raw_orders\n    managed: true\n",
                "python/loaders/generated.py": """
from sqlbuild.factories import factory
from sqlbuild.loaders import loader


@factory
def generated_loaders():
    @loader(name="raw_orders")
    def load(ctx):
        return [{"order_id": 1}]
    return load
""",
            },
            expected_loader_names=("raw_orders",),
        ),
        DiscoverFactoryValidationTestCase(
            description="generated task and asset dependency validates",
            repo_files=base_repo_files()
            | {
                "python/factories/generated.py": """
from sqlbuild.assets import asset
from sqlbuild.factories import factory
from sqlbuild.tasks import task


@factory
def generated_nodes():
    @task(name="prepare_orders")
    def prepare(ctx):
        return None

    @asset(name="orders_export", depends_on=prepare)
    def export(ctx):
        return None

    return [prepare, export]
""",
            },
            expected_task_names=("prepare_orders",),
            expected_asset_names=("orders_export",),
        ),
        DiscoverFactoryValidationTestCase(
            description="single-kind factory validates in factories folder",
            repo_files=base_repo_files()
            | {
                "python/factories/generated_tasks.py": """
from sqlbuild.factories import factory
from sqlbuild.tasks import task


@factory
def generated_tasks():
    @task(name="prepare_orders")
    def prepare(ctx):
        return None

    return prepare
""",
            },
            expected_task_names=("prepare_orders",),
        ),
        DiscoverFactoryValidationTestCase(
            description="factory returning its own module-level node registers it once",
            repo_files=base_repo_files()
            | {
                "python/factories/orders.py": """
from sqlbuild.factories import factory
from sqlbuild.tasks import task


@task
def orders(ctx):
    return None


@factory
def order_nodes():
    return orders
""",
            },
            expected_task_names=("orders",),
        ),
        DiscoverFactoryValidationTestCase(
            description="factory binding its generated node to a module global registers it once",
            repo_files=base_repo_files()
            | {
                "python/factories/orders.py": """
from sqlbuild.factories import factory
from sqlbuild.tasks import task


@factory
def order_nodes():
    global orders

    @task
    def orders(ctx):
        return None

    return orders
""",
            },
            expected_task_names=("orders",),
        ),
        DiscoverFactoryValidationTestCase(
            description="factory returning an imported python module node registers it once",
            repo_files=base_repo_files()
            | {
                "python/a_factories/orders.py": """
from python.tasks.orders import prepare_orders
from sqlbuild.factories import factory
from sqlbuild.tasks import task


@factory
def order_nodes():
    @task(name="publish_orders", depends_on=prepare_orders)
    def publish(ctx):
        return None
    return [prepare_orders, publish]
""",
                "python/tasks/orders.py": """
from sqlbuild.tasks import task


@task
def prepare_orders(ctx):
    return None
""",
            },
            expected_task_names=("prepare_orders", "publish_orders"),
        ),
        DiscoverFactoryValidationTestCase(
            description="factory can import ordinary private helper module",
            repo_files=base_repo_files()
            | {
                "python/factories/orders/_helpers.py": "TASK_NAME = 'prepare_orders'\n",
                "python/factories/orders/generated.py": """
from python.factories.orders._helpers import TASK_NAME
from sqlbuild.factories import factory
from sqlbuild.tasks import task


@factory
def generated_tasks():
    @task(name=TASK_NAME)
    def prepare(ctx):
        return None

    return prepare
""",
            },
            expected_task_names=("prepare_orders",),
        ),
        DiscoverFactoryValidationTestCase(
            description="generated check and task may depend on a model and a seed",
            repo_files=base_repo_files()
            | {
                "python/factories/generated.py": """
from sqlbuild.checks import check
from sqlbuild.factories import factory
from sqlbuild.refs import model, seed
from sqlbuild.tasks import task


@factory
def generated_nodes():
    @task(name="export_countries", depends_on=seed("country_codes"))
    def export_countries(ctx):
        return None

    @check(name="check_orders", depends_on=[model("orders"), seed("country_codes")])
    def check_orders(ctx):
        return True

    return [export_countries, check_orders]
""",
            },
            expected_task_names=("export_countries",),
            expected_check_names=("check_orders",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_generated_factory_nodes_when_discovering_inputs_then_validates_expanded_nodes(
    test_case: DiscoverFactoryValidationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    result: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)

    assert (
        tuple(loader.name for loader in result.loader_functions) == test_case.expected_loader_names
    )
    assert tuple(task.name for task in result.task_functions) == test_case.expected_task_names
    assert tuple(asset.name for asset in result.asset_functions) == test_case.expected_asset_names
    assert tuple(check.name for check in result.check_functions) == test_case.expected_check_names


@pytest.mark.parametrize(
    "test_case",
    (
        ProjectPythonPathAcceptanceTestCase(
            description="undecorated helper modules under python root",
            repo_files={
                "python/_helpers.py": "REGION = 'emea'\n",
                "python/orders/__init__.py": "",
                "python/orders/utils.py": "def order_label(value):\n    return str(value)\n",
                "adapter.py": "ADAPTER = None\n",
            },
            expected_project_name="demo",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_supported_project_python_when_discovering_inputs_then_paths_are_accepted(
    test_case: ProjectPythonPathAcceptanceTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, base_repo_files() | test_case.repo_files)

    result: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)

    assert result.project_config.name == test_case.expected_project_name


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoverProjectInputsErrorTestCase(
            description="raises when Python lives in removed tasks root",
            repo_files=base_repo_files() | {"tasks/orders.py": "VALUE = 1\n"},
            expected_error_fragment="tasks/orders.py",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when Python lives in removed assets root",
            repo_files=base_repo_files() | {"assets/orders.py": "VALUE = 1\n"},
            expected_error_fragment="assets/orders.py",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when Python lives in removed checks root",
            repo_files=base_repo_files() | {"checks/orders.py": "VALUE = 1\n"},
            expected_error_fragment="checks/orders.py",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when Python lives in removed loaders root",
            repo_files=base_repo_files() | {"loaders/orders.py": "VALUE = 1\n"},
            expected_error_fragment="loaders/orders.py",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when Python lives in removed factories root",
            repo_files=base_repo_files() | {"factories/orders.py": "VALUE = 1\n"},
            expected_error_fragment="factories/orders.py",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when Python lives in removed libs root",
            repo_files=base_repo_files() | {"libs/cleaning.py": "VALUE = 1\n"},
            expected_error_fragment="libs/cleaning.py",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when Python lives in removed dagster root",
            repo_files=base_repo_files() | {"dagster/definitions.py": "VALUE = 1\n"},
            expected_error_fragment="dagster/definitions.py",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when Python lives in removed rivers_pipeline root",
            repo_files=base_repo_files() | {"rivers_pipeline/definitions.py": "VALUE = 1\n"},
            expected_error_fragment="rivers_pipeline/definitions.py",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when Python lives in removed root definitions root",
            repo_files=base_repo_files() | {"definitions.py": "VALUE = 1\n"},
            expected_error_fragment="definitions.py",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_python_in_removed_root_when_discovering_inputs_then_d016_is_raised(
    test_case: DiscoverProjectInputsErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    with pytest.raises(ProjectPythonPathError) as error_info:
        discover_project_inputs(project_dir=tmp_path)

    assert error_info.value.code == "D016"
    assert f"Unsupported project Python path(s): {test_case.expected_error_fragment}" in str(
        error_info.value
    )


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoverProjectInputsErrorTestCase(
            description="unselected project target without connection among several",
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
                    '[connections.local]\ndatabase = "demo.duckdb"\n\n'
                    '[connections.archive]\ndatabase = "archive.duckdb"\n\n'
                    '[targets.dev]\nconnection = "local"\nschema = "dev"\n\n'
                    '[targets.prod]\nschema = "prod"\n'
                )
            },
            expected_error_fragment=(
                "targets.prod does not set connection and several named connections "
                "are defined (archive, local)"
            ),
        ),
        DiscoverProjectInputsErrorTestCase(
            description="local connection makes a project target ambiguous",
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\nadapter = "duckdb"\n\n'
                    '[connections.local]\ndatabase = "demo.duckdb"\n\n'
                    '[targets.dev]\nschema = "dev"\n'
                ),
                "sqlbuild_local.toml": '[connections.scratch]\ndatabase = "scratch.duckdb"\n',
            },
            expected_error_fragment="several named connections are defined (local, scratch)",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_ambiguous_target_connection_when_discovering_then_config_error_is_raised(
    test_case: DiscoverProjectInputsErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    with pytest.raises(ProjectConfigError) as error_info:
        discover_project_inputs(project_dir=tmp_path)

    assert error_info.value.code == "D001"
    assert test_case.expected_error_fragment in str(error_info.value)
    assert str(tmp_path / "sqlbuild_project.toml") in str(error_info.value)


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoverProjectInputsErrorTestCase(
            description="non-project directory containing Python",
            repo_files={"application/main.py": "print('hello')\n"},
            expected_error_fragment="Project config not found",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_non_project_with_python_when_discovering_then_missing_config_error_takes_precedence(
    test_case: DiscoverProjectInputsErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        discover_project_inputs(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoverFactoryValidationTestCase(
            description="generated duplicate names fail validation",
            repo_files=base_repo_files()
            | {
                "python/tasks/generated.py": """
from sqlbuild.factories import factory
from sqlbuild.tasks import task


def make_task():
    @task(name="profile")
    def profile(ctx):
        return None
    return profile


@factory
def generated_tasks():
    return [make_task(), make_task()]
""",
            },
            expected_error_fragment="Duplicate Python node found for 'profile'",
        ),
        DiscoverFactoryValidationTestCase(
            description="generated name colliding with top-level node fails validation",
            repo_files=base_repo_files()
            | {
                "python/tasks/top_level.py": """
from sqlbuild.tasks import task


@task(name="profile")
def top_level_profile(ctx):
    return None
""",
                "python/assets/generated.py": """
from sqlbuild.assets import asset
from sqlbuild.factories import factory


@factory
def generated_assets():
    @asset(name="profile")
    def generated_profile(ctx):
        return None
    return generated_profile
""",
            },
            expected_error_fragment="Duplicate Python node found for 'profile'",
        ),
        DiscoverFactoryValidationTestCase(
            description="factory returning a distinct node named like a module node fails validation",
            repo_files=base_repo_files()
            | {
                "python/factories/orders.py": """
from sqlbuild.factories import factory
from sqlbuild.tasks import task


@task
def orders(ctx):
    return None


@factory
def order_nodes():
    @task(name="orders")
    def generated_orders(ctx):
        return None
    return generated_orders
""",
            },
            expected_error_fragment="Duplicate Python node found for 'orders'",
        ),
        DiscoverFactoryValidationTestCase(
            description="generated dependency cycle fails validation",
            repo_files=base_repo_files()
            | {
                "python/tasks/generated.py": """
from sqlbuild.factories import factory
from sqlbuild.tasks import task


@factory
def generated_tasks():
    def first_body(ctx):
        return None
    def second_body(ctx):
        return None
    first = task(name="first", depends_on=[])(first_body)
    second = task(name="second", depends_on=[first])(second_body)
    first = task(name="first", depends_on=[second])(first)
    return [first, second]
""",
            },
            expected_error_fragment="Python node dependency cycle detected",
        ),
        DiscoverFactoryValidationTestCase(
            description="generated loader SQL dependency fails validation",
            repo_files=base_repo_files()
            | {
                "python/loaders/generated.py": """
from sqlbuild.factories import factory
from sqlbuild.loaders import loader
from sqlbuild.refs import model


@factory
def generated_loaders():
    @loader(name="raw_orders", depends_on=[model("orders")])
    def raw_orders(ctx):
        return []
    return raw_orders
""",
            },
            expected_error_fragment="Loader 'raw_orders' depends on SQL resource 'orders'",
        ),
        DiscoverFactoryValidationTestCase(
            description="generated managed source loader mismatch fails validation",
            repo_files=base_repo_files()
            | {
                "sources/raw.yml": "sources:\n  - name: raw_orders\n    managed: true\n",
                "python/loaders/generated.py": """
from sqlbuild.factories import factory
from sqlbuild.loaders import loader


@factory
def generated_loaders():
    @loader(name="load_orders")
    def load(ctx):
        return []
    return load
""",
            },
            expected_error_fragment="Managed source 'raw_orders' in sources/raw.yml requires loader",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_generated_factory_nodes_when_discovering_inputs_then_raises(
    test_case: DiscoverFactoryValidationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        discover_project_inputs(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverUniqueTestNameTestCase(
            description="parameterized cases share their test's unique name",
            repo_files=base_repo_files()
            | {
                "models/marts/orders.sql": "MODEL ();\n\nselect 1\n",
                "tests/unit/order_status.sql": (
                    "TEST (parameters (status string), cases "
                    '(open_case (status "open"), closed_case (status "closed")));\nSELECT 1\n'
                ),
            },
            expected_test_block_names=(None,),
            expected_case_names=("open_case", "closed_case"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_parameterized_test_when_discovering_inputs_then_cases_are_not_duplicate_names(
    test_case: DiscoverUniqueTestNameTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)

    blocks: tuple[DiscoveredSqlTestBlock, ...] = discovered.test_files[0].blocks
    assert tuple(block.name for block in blocks) == test_case.expected_test_block_names
    assert tuple(case.name for case in blocks[0].cases) == test_case.expected_case_names


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoverProjectInputsErrorTestCase(
            description="raises when Python lives outside a supported project root",
            repo_files=base_repo_files()
            | {"audit_helpers/measurements.py": "def build_cases(): return []\n"},
            expected_error_fragment=(
                "Unsupported project Python path\\(s\\): audit_helpers/measurements.py"
            ),
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises on duplicate source names across files",
            repo_files=base_repo_files()
            | {
                "sources/raw_orders.yml": """
sources:
  - name: raw_orders
    schema: public
    table: orders
""".strip()
                + "\n",
                "sources/raw_orders_duplicate.yml": """
sources:
  - name: raw_orders
    schema: public
    table: orders_backup
""".strip()
                + "\n",
            },
            expected_error_fragment="Duplicate source declaration found for 'raw_orders'",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises on schema yml model metadata",
            repo_files=base_repo_files()
            | {
                "models/staging/schema.yml": """
models:
  - name: stg_orders
""".strip()
                + "\n",
            },
            expected_error_fragment="model metadata must live in the model file MODEL",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises on duplicate schema seed names across files",
            repo_files=base_repo_files()
            | {
                "seeds/a.yml": """
seeds:
  - name: country_codes
    columns:
      - name: country_code
        type: VARCHAR
""".strip()
                + "\n",
                "seeds/b.yml": """
seeds:
  - name: country_codes
    columns:
      - name: country_code
        type: VARCHAR
""".strip()
                + "\n",
            },
            expected_error_fragment="Duplicate seed declaration found for 'country_codes'",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises on duplicate model file names across directories",
            repo_files=base_repo_files()
            | {
                "models/staging/orders.sql": "MODEL ();\n\nselect 1\n",
                "models/marts/orders.sql": "MODEL ();\n\nselect 1\n",
            },
            expected_error_fragment="Duplicate model file name found for 'orders'",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises on duplicate scenario file names across directories",
            repo_files=base_repo_files()
            | {
                "tests/scenarios/revenue/customer_refund.sql": "SCENARIO ();\nSELECT 1\n",
                "tests/scenarios/support/customer_refund.sql": "SCENARIO ();\nSELECT 1\n",
            },
            expected_error_fragment="Duplicate scenario file name found for 'customer_refund'",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when a unit test and a model share a name",
            repo_files=base_repo_files()
            | {
                "models/marts/orders.sql": "MODEL ();\n\nselect 1\n",
                "tests/unit/orders.sql": "TEST ();\nSELECT 1\n",
            },
            expected_error_fragment=(
                "Project resource name 'orders' is declared as both model in "
                "models/marts/orders.sql and unit test in tests/unit/orders.sql; .*unit test, "
                "scenario, job, job group, sensor, and calendar schedule names must be globally unique"
            ),
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when a scenario and a source share a name",
            repo_files=base_repo_files()
            | {
                "sources/raw.yml": (
                    "sources:\n  - name: raw_orders\n    schema: public\n    table: orders\n"
                ),
                "tests/scenarios/raw_orders.sql": "SCENARIO ();\nSELECT 1\n",
            },
            expected_error_fragment=(
                "Project resource name 'raw_orders' is declared as both source in "
                "sources/raw.yml and scenario in tests/scenarios/raw_orders.sql"
            ),
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when a named unit test and a scenario share a name",
            repo_files=base_repo_files()
            | {
                "tests/scenarios/order_refunds.sql": "SCENARIO ();\nSELECT 1\n",
                "tests/unit/refunds.sql": 'TEST (name "order_refunds");\nSELECT 1\n',
            },
            expected_error_fragment=(
                "Project resource name 'order_refunds' is declared as both scenario in "
                "tests/scenarios/order_refunds.sql and unit test in tests/unit/refunds.sql"
            ),
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when model and source names collide",
            repo_files=base_repo_files()
            | {
                "models/staging/orders.sql": "MODEL ();\n\nselect 1\n",
                "sources/raw.yml": """
sources:
  - name: orders
    table: orders
""".strip()
                + "\n",
            },
            expected_error_fragment="Logical relation name 'orders' is declared as both model",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when source and seed names collide",
            repo_files=base_repo_files()
            | {
                "sources/raw.yml": """
sources:
  - name: country_codes
    table: country_codes
""".strip()
                + "\n",
                "seeds/schema.yml": """
seeds:
  - name: country_codes
    columns:
      - name: country_code
        type: VARCHAR
""".strip()
                + "\n",
            },
            expected_error_fragment="Logical relation name 'country_codes' is declared as both source",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when a source references an unknown loader",
            repo_files=base_repo_files()
            | {
                "sources/raw.yml": """
sources:
  - name: raw_orders
    managed: true
""".strip()
                + "\n",
            },
            expected_error_fragment="Managed source 'raw_orders' in sources/raw.yml requires loader",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when loader names are duplicated",
            repo_files=base_repo_files()
            | {
                "python/loaders/a.py": """
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    return []
""",
                "python/loaders/b.py": """
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    return []
""",
            },
            expected_error_fragment="Duplicate source loader found for 'raw_orders'",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when explicit loader names are duplicated",
            repo_files=base_repo_files()
            | {
                "python/loaders/a.py": """
from sqlbuild.loaders import loader

@loader(name="raw_orders")
def load_a(ctx):
    return []
""",
                "python/loaders/b.py": """
from sqlbuild.loaders import loader

@loader(name="raw_orders")
def load_b(ctx):
    return []
""",
            },
            expected_error_fragment="Duplicate source loader found for 'raw_orders'",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when unmanaged source name collides with loader name",
            repo_files=base_repo_files()
            | {
                "sources/raw.yml": """
sources:
  - name: fetch_orders
    table: fetch_orders
""".strip()
                + "\n",
                "python/loaders/raw_orders.py": """
from sqlbuild.loaders import loader

@loader
def fetch_orders(ctx):
    return []
""",
            },
            expected_error_fragment="Source 'fetch_orders' in sources/raw.yml conflicts with loader",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when unmanaged source name collides with terminal loader name",
            repo_files=base_repo_files()
            | {
                "sources/raw.yml": """
sources:
  - name: raw_orders
    table: raw_orders
""".strip()
                + "\n",
                "python/loaders/raw_orders.py": """
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    return []
""",
            },
            expected_error_fragment="Source 'raw_orders' in sources/raw.yml conflicts with loader",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when loader dependency is not decorated",
            repo_files=base_repo_files()
            | {
                "python/loaders/events.py": """
from sqlbuild.loaders import loader

def fetch_events(ctx):
    return []

@loader(depends_on=[fetch_events])
def enriched_events(ctx):
    return []
""",
            },
            expected_error_fragment="Loader 'enriched_events' depends on an unknown loader",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when loader dependencies contain a cycle",
            repo_files=base_repo_files()
            | {
                "python/loaders/events.py": """
from sqlbuild.loaders import loader

def fetch_events(ctx):
    return []

@loader(depends_on=[fetch_events])
def enriched_events(ctx):
    return []

fetch_events = loader(depends_on=[enriched_events])(fetch_events)
""",
            },
            expected_error_fragment="Loader dependency cycle detected",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when terminal loader owns source config",
            repo_files=base_repo_files()
            | {
                "sources/raw.yml": """
sources:
  - name: raw_orders
    managed: true
    write_strategy: table
""".strip()
                + "\n",
                "python/loaders/raw_orders.py": """
from sqlbuild.loaders import loader

@loader(write_strategy="table", columns=[{"name": "id", "type": "INTEGER"}])
def raw_orders(ctx):
    return []
""",
            },
            expected_error_fragment="terminal source loader write and schema config must be declared",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when task and asset names are duplicated",
            repo_files=base_repo_files()
            | {
                "python/tasks/export_customers.py": """
from sqlbuild.tasks import task

@task
def export_customers(ctx):
    return None
""",
                "python/assets/export_customers.py": """
from sqlbuild.assets import asset

@asset
def export_customers(ctx):
    return None
""",
            },
            expected_error_fragment="Duplicate Python node found for 'export_customers'",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when check name collides with task name",
            repo_files=base_repo_files()
            | {
                "python/tasks/export_customers.py": """
from sqlbuild.tasks import task

@task
def export_customers(ctx):
    return None
""",
                "python/checks/export_customers.py": """
from sqlbuild.checks import check
from python.tasks.export_customers import export_customers

@check(depends_on=export_customers, name="export_customers")
def export_customers_check(ctx):
    return True
""",
            },
            expected_error_fragment="Duplicate Python node found for 'export_customers'",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when model name collides with task name",
            repo_files=base_repo_files()
            | {
                "models/marts/export_customers.sql": "MODEL ();\n\nselect 1\n",
                "python/tasks/export_customers.py": """
from sqlbuild.tasks import task

@task
def export_customers(ctx):
    return None
""",
            },
            expected_error_fragment=(
                "Project resource name 'export_customers' is declared as both model"
            ),
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when seed name collides with asset name",
            repo_files=base_repo_files()
            | {
                "seeds/schema.yml": """
seeds:
  - name: export_customers
    columns:
      - name: customer_id
        type: INTEGER
""".strip()
                + "\n",
                "seeds/export_customers.csv": "customer_id\n1\n",
                "python/assets/export_customers.py": """
from sqlbuild.assets import asset

@asset
def export_customers(ctx):
    return None
""",
            },
            expected_error_fragment=(
                "Project resource name 'export_customers' is declared as both seed"
            ),
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when source name collides with check name",
            repo_files=base_repo_files()
            | {
                "sources/raw.yml": """
sources:
  - name: export_customers_exists
    table: export_customers_exists
""".strip()
                + "\n",
                "python/tasks/export_customers.py": """
from sqlbuild.tasks import task

@task
def export_customers(ctx):
    return None
""",
                "python/checks/export_customers.py": """
from sqlbuild.checks import check
from python.tasks.export_customers import export_customers

@check(depends_on=export_customers)
def export_customers_exists(ctx):
    return True
""",
            },
            expected_error_fragment=(
                "Project resource name 'export_customers_exists' is declared as both source"
            ),
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when sql function name collides with python function name",
            repo_files=base_repo_files()
            | {
                "functions/sql/is_large_order.sql": """
FUNCTION (
  arguments (amount INTEGER),
  returns BOOLEAN,
);

amount > 100
""".strip()
                + "\n",
                "functions/python/is_large_order.py": """
from sqlbuild.functions import udf

@udf(arguments={"amount": "INTEGER"}, returns="BOOLEAN", runtime_version="3.11")
def main(amount):
    return amount > 100
""".strip()
                + "\n",
            },
            expected_error_fragment=(
                "Project resource name 'is_large_order' is declared as both function"
            ),
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when function name collides with loader name",
            repo_files=base_repo_files()
            | {
                "functions/sql/fetch_orders.sql": """
FUNCTION (
  arguments (amount INTEGER),
  returns BOOLEAN,
);

amount > 100
""".strip()
                + "\n",
                "python/loaders/fetch_orders.py": """
from sqlbuild.loaders import loader

@loader
def fetch_orders(ctx):
    return []
""",
            },
            expected_error_fragment=(
                "Project resource name 'fetch_orders' is declared as both function"
            ),
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when SQL hook name collides with model name",
            repo_files=base_repo_files()
            | {
                "models/notify.sql": "MODEL ();\n\nSELECT 1\n",
                "hooks/sql/notify.sql": "HOOK ();\n\nSELECT 1\n",
            },
            expected_error_fragment=(
                "Project resource name 'notify' is declared as both model in models/notify.sql "
                "and sql hook in hooks/sql/notify.sql"
            ),
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when Python hook name collides with provider name",
            repo_files=base_repo_files()
            | {
                "providers/marker.py": """
from sqlbuild.providers import Provider

class MarkerProvider(Provider):
    pass
""",
                "hooks/python/marker.py": """
from sqlbuild.hooks import hook

@hook(name="marker_provider")
def marker(ctx):
    return None
""",
            },
            expected_error_fragment=(
                "Project resource name 'marker_provider' is declared as both provider"
            ),
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when SQL and Python hook names collide",
            repo_files=base_repo_files()
            | {
                "hooks/sql/notify.sql": "HOOK ();\n\nSELECT 1\n",
                "hooks/python/notify.py": """
from sqlbuild.hooks import hook

@hook
def notify(ctx):
    return None
""",
            },
            expected_error_fragment=(
                "Project resource name 'notify' is declared as both sql hook in "
                "hooks/sql/notify.sql and python hook in hooks/python/notify.py"
            ),
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when task dependency is not decorated",
            repo_files=base_repo_files()
            | {
                "python/tasks/windows.py": """
from sqlbuild.tasks import task

def fetch_window(ctx):
    return None

@task(depends_on=fetch_window)
def export_window(ctx):
    return None
""",
            },
            expected_error_fragment="Python node 'export_window' depends on an unknown Python node",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when task and asset dependencies contain a cycle",
            repo_files=base_repo_files()
            | {
                "python/tasks/windows.py": """
from sqlbuild.tasks import task

def fetch_window(ctx):
    return None

@task(depends_on=fetch_window)
def enrich_window(ctx):
    return None

fetch_window = task(depends_on=enrich_window)(fetch_window)
""",
            },
            expected_error_fragment="Python node dependency cycle detected",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when check dependency is not decorated",
            repo_files=base_repo_files()
            | {
                "python/checks/exports.py": """
from sqlbuild.checks import check

def export_customers(ctx):
    return None

@check(depends_on=export_customers)
def export_customers_exists(ctx):
    return True
""",
            },
            expected_error_fragment="Check 'export_customers_exists' depends on an unknown Python node",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when loader declares SQL model dependency",
            repo_files=base_repo_files()
            | {
                "python/loaders/orders.py": """
from sqlbuild.loaders import loader
from sqlbuild.refs import model

@loader(depends_on=[model('stg_orders')])
def load_orders(ctx):
    return []
""",
            },
            expected_error_fragment="Loader 'load_orders' depends on SQL resource 'stg_orders'",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when check depends on check",
            repo_files=base_repo_files()
            | {
                "python/assets/exports.py": """
from sqlbuild.assets import asset

@asset
def export_customers(ctx):
    return None
""",
                "python/checks/exports.py": """
from sqlbuild.checks import check
from python.assets.exports import export_customers

@check(depends_on=export_customers)
def export_customers_exists(ctx):
    return True

@check(depends_on=export_customers_exists)
def export_customers_recent(ctx):
    return True
""",
            },
            expected_error_fragment="Check 'export_customers_recent' depends on another check",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when seeds are declared outside seeds directory",
            repo_files=base_repo_files()
            | {
                "models/schema.yml": """
seeds:
  - name: country_codes
    columns:
      - name: country_code
        type: VARCHAR
""".strip()
                + "\n",
            },
            expected_error_fragment="seed declarations must live under seeds/",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when a seed declaration uses yaml extension",
            repo_files=base_repo_files()
            | {
                "seeds/lookups.yaml": "seeds: []\n",
            },
            expected_error_fragment=r"\.yaml is not supported",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when seed csv basenames are duplicated",
            repo_files=base_repo_files()
            | {
                "seeds/a/country_codes.csv": "country_code\nUS\n",
                "seeds/b/country_codes.csv": "country_code\nCA\n",
                "seeds/lookups.yml": """
seeds:
  - name: country_codes
    columns:
      - name: country_code
        type: VARCHAR
""".strip()
                + "\n",
            },
            expected_error_fragment="Duplicate seed CSV name 'country_codes' found",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when a declared seed has no matching csv file",
            repo_files=base_repo_files()
            | {
                "seeds/schema.yml": """
seeds:
  - name: country_codes
    columns:
      - name: country_code
        type: VARCHAR
""".strip()
                + "\n",
            },
            expected_error_fragment="has no matching CSV file under seeds/",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when a seed csv has no matching declaration",
            repo_files=base_repo_files()
            | {
                "seeds/country_codes.csv": "country_code\nUS\n",
            },
            expected_error_fragment="has no matching declaration for seed 'country_codes'",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when a seed csv header does not match declared columns",
            repo_files=base_repo_files()
            | {
                "seeds/schema.yml": """
seeds:
  - name: country_codes
    columns:
      - name: country_code
        type: VARCHAR
      - name: country_name
        type: VARCHAR
""".strip()
                + "\n",
                "seeds/country_codes.csv": "country_name,country_code\nUS,United States\n",
            },
            expected_error_fragment="does not match declared seed columns",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when a seed csv has duplicate header columns",
            repo_files=base_repo_files()
            | {
                "seeds/schema.yml": """
seeds:
  - name: country_codes
    columns:
      - name: country_code
        type: VARCHAR
      - name: country_name
        type: VARCHAR
""".strip()
                + "\n",
                "seeds/country_codes.csv": "country_code,country_code\nUS,United States\n",
            },
            expected_error_fragment="contains duplicate CSV header column 'country_code'",
        ),
        DiscoverProjectInputsErrorTestCase(
            description="raises when path defaults match no model paths",
            repo_files=base_repo_files()
            | {
                "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"

[path_defaults.stagingg]
schema = "staging"
""".strip()
                + "\n",
                "models/staging/orders.sql": "MODEL ();\n\nselect 1\n",
            },
            expected_error_fragment=r"path_defaults\['stagingg'\] does not match any model paths",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_discovery_conflicts_when_discovering_inputs_then_it_raises_clear_errors(
    test_case: DiscoverProjectInputsErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        discover_project_inputs(project_dir=tmp_path)
