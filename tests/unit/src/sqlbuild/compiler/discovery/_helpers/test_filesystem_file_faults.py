"""Tests for per-file fault isolation and module import failures in filesystem discovery."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveryFileFault,
    EventExporterDiscoveryError,
    ProviderDiscoveryError,
    PythonNodeDiscoveryError,
    discover_audit_files,
    discover_constant_files,
    discover_enum_files,
    discover_event_exporter_declarations,
    discover_materialization_files,
    discover_provider_classes,
    discover_scenario_files,
    discover_source_files,
    discover_sql_function_files,
    discover_sql_hook_files,
)
from sqlbuild.compiler.discovery._helpers.filesystem.command_output_sinks import (
    discover_command_output_sink_declarations,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    DiscoverFileFaultsTestCase,
    DiscoverModuleImportFailureTestCase,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers.helpers import write_unreadable_files

_FILE_DISCOVERIES: dict[str, Callable[..., tuple[object, ...]]] = {
    "audits": discover_audit_files,
    "constants": discover_constant_files,
    "enums": discover_enum_files,
    "scenarios": discover_scenario_files,
    "sources": discover_source_files,
    "sql_functions": discover_sql_function_files,
    "sql_hooks": discover_sql_hook_files,
}
_MODULE_DISCOVERIES: dict[str, Callable[..., object]] = {
    "command_output_sinks": discover_command_output_sink_declarations,
    "event_exporters": discover_event_exporter_declarations,
    "materializations": discover_materialization_files,
    "providers": discover_provider_classes,
}


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoverFileFaultsTestCase(
            description="audits fault every nested SQL file",
            discovery_name="audits",
            unreadable_files=(
                "sql/audits/singular/orders.sql",
                "sql/audits/generic/nested/_customers.sql",
            ),
            expected_fault_paths=(
                Path("sql/audits/generic/nested/_customers.sql"),
                Path("sql/audits/singular/orders.sql"),
            ),
        ),
        DiscoverFileFaultsTestCase(
            description="constants fault declaration files",
            discovery_name="constants",
            unreadable_files=("sql/constants/order_limits.sql",),
            expected_fault_paths=(Path("sql/constants/order_limits.sql"),),
        ),
        DiscoverFileFaultsTestCase(
            description="enums fault declaration files",
            discovery_name="enums",
            unreadable_files=("sql/enums/order_status.sql",),
            expected_fault_paths=(Path("sql/enums/order_status.sql"),),
        ),
        DiscoverFileFaultsTestCase(
            description="scenarios fault nested SQL files",
            discovery_name="scenarios",
            unreadable_files=("tests/scenarios/orders/_refunds.sql",),
            expected_fault_paths=(Path("tests/scenarios/orders/_refunds.sql"),),
        ),
        DiscoverFileFaultsTestCase(
            description="sources fault only top-level YAML files",
            discovery_name="sources",
            unreadable_files=(
                "sql/sources/orders.yml",
                "sql/sources/customers.yaml",
                "sql/sources/notes.txt",
                "sql/sources/nested/products.yml",
            ),
            expected_fault_paths=(
                Path("sql/sources/customers.yaml"),
                Path("sql/sources/orders.yml"),
            ),
        ),
        DiscoverFileFaultsTestCase(
            description="sql functions fault underscore-prefixed files",
            discovery_name="sql_functions",
            unreadable_files=("sql/functions/sql/_order_total.sql", "sql/functions/sql/tax.sql"),
            expected_fault_paths=(
                Path("sql/functions/sql/_order_total.sql"),
                Path("sql/functions/sql/tax.sql"),
            ),
        ),
        DiscoverFileFaultsTestCase(
            description="sql hooks skip underscore-prefixed files",
            discovery_name="sql_hooks",
            unreadable_files=("sql/hooks/sql/_shared.sql", "sql/hooks/sql/grants/grant_access.sql"),
            expected_fault_paths=(Path("sql/hooks/sql/grants/grant_access.sql"),),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unreadable_files_when_discovering_then_faults_each_file_or_raises(
    tmp_path: Path,
    test_case: DiscoverFileFaultsTestCase,
) -> None:
    write_unreadable_files(project_dir=tmp_path, relative_paths=test_case.unreadable_files)
    discover: Callable[..., tuple[object, ...]] = _FILE_DISCOVERIES[test_case.discovery_name]
    faults: list[DiscoveryFileFault] = []

    discovered: tuple[object, ...] = discover(project_dir=tmp_path, on_fault=faults.append)

    assert discovered == ()
    assert tuple(fault.path for fault in faults) == test_case.expected_fault_paths
    assert all("utf-8" in fault.message for fault in faults)
    with pytest.raises(test_case.expected_error_type):
        _ = discover(project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    (
        DiscoverModuleImportFailureTestCase(
            description="command output sink import failure identifies the sink file",
            discovery_name="command_output_sinks",
            broken_file="python/event_sinks/broken_orders.py",
            expected_error_type=EventExporterDiscoveryError,
            expected_error_fragment="Failed to import sink file python/event_sinks/broken_orders.py: ",
        ),
        DiscoverModuleImportFailureTestCase(
            description="lifecycle sink import failure identifies the sink file",
            discovery_name="event_exporters",
            broken_file="python/event_sinks/broken_orders.py",
            expected_error_type=EventExporterDiscoveryError,
            expected_error_fragment="Failed to import sink file python/event_sinks/broken_orders.py: ",
        ),
        DiscoverModuleImportFailureTestCase(
            description="materialization import failure identifies the materialization file",
            discovery_name="materializations",
            broken_file="python/materializations/broken_orders.py",
            expected_error_type=PythonNodeDiscoveryError,
            expected_error_fragment=(
                "Failed to import materialization file python/materializations/broken_orders.py: "
            ),
        ),
        DiscoverModuleImportFailureTestCase(
            description="provider import failure identifies the provider file",
            discovery_name="providers",
            broken_file="python/providers/broken_orders.py",
            expected_error_type=ProviderDiscoveryError,
            expected_error_fragment="Failed to import provider file python/providers/broken_orders.py: ",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_module_import_failure_when_discovering_then_raises_kind_specific_error(
    tmp_path: Path,
    test_case: DiscoverModuleImportFailureTestCase,
) -> None:
    file_path: Path = tmp_path / test_case.broken_file
    file_path.parent.mkdir(parents=True)
    _ = file_path.write_text("raise RuntimeError('orders module failed')\n", encoding="utf-8")

    with pytest.raises(test_case.expected_error_type) as error_info:
        _ = _MODULE_DISCOVERIES[test_case.discovery_name](project_dir=tmp_path)

    assert str(error_info.value) == f"{test_case.expected_error_fragment}orders module failed"


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
