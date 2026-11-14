"""Tests for sqb load selector resolution."""

from __future__ import annotations

from dataclasses import replace

import pytest

from sqlbuild._native import (
    DefaultsConfig,
    DiscoveredLoaderFunction,
    DiscoveredProjectInputs,
    LocalConfig,
    ProjectConfig,
    SourceEntry,
    TargetConfig,
    resolve_loader_defaults,
    select_load_entries,
    select_load_reference_entries,
)
from tests.unit.src.sqlbuild.cli.commands._helpers.load_selection._test_types import (
    LoaderDefaultResolutionTestCase,
    LoaderDestinationSelectionTestCase,
    LoadReferenceSelectionTestCase,
    LoadSelectionTestCase,
)
from tests.unit.src.sqlbuild.cli.commands._helpers.load_selection.helpers import (
    build_integration_load_selection_inputs,
    build_load_selection_inputs,
)


@pytest.mark.parametrize(
    "test_case",
    [
        LoadSelectionTestCase(
            description="source selector selects terminal source load only",
            select=("raw_orders",),
            expected_entry_names=("raw_orders",),
            expected_loader_node_flags=(False,),
        ),
        LoadSelectionTestCase(
            description="leading plus includes upstream intermediate loader",
            select=("+raw_orders",),
            expected_entry_names=("fetch_orders", "raw_orders"),
            expected_loader_node_flags=(True, False),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_load_selector_when_selecting_then_returns_expected_source_entries(
    test_case: LoadSelectionTestCase,
) -> None:
    entries: tuple[SourceEntry, ...] = select_load_entries(
        discovered_inputs=build_load_selection_inputs(),
        select=test_case.select,
        exclude=(),
        target_config=None,
    )

    assert tuple(entry.name for entry in entries) == test_case.expected_entry_names
    assert (
        tuple(entry.meta.get("sqlbuild_loader_node") is True for entry in entries)
        == test_case.expected_loader_node_flags
    )


@pytest.mark.parametrize(
    "test_case",
    [
        LoadReferenceSelectionTestCase(
            description="integration terminal loader is selected, not reference-only",
            select=("raw_orders",),
            expected_entry_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_integration_loader_selected_when_selecting_references_then_returns_no_terminal_ref(
    test_case: LoadReferenceSelectionTestCase,
) -> None:
    discovered_inputs: DiscoveredProjectInputs = build_integration_load_selection_inputs()
    selected_entries: tuple[SourceEntry, ...] = select_load_entries(
        discovered_inputs=discovered_inputs,
        select=test_case.select,
        exclude=(),
        target_config=None,
    )

    reference_entries: tuple[SourceEntry, ...] = select_load_reference_entries(
        discovered_inputs=discovered_inputs,
        selected_sources=selected_entries,
        target_config=None,
    )

    assert tuple(entry.name for entry in reference_entries) == test_case.expected_entry_names


@pytest.mark.parametrize(
    "test_case",
    (
        LoaderDestinationSelectionTestCase(
            description="two-part destination uses effective database default",
            destination="staging.orders",
            expected_parts=("connection_db", "staging", "orders"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_intermediate_loader_when_selecting_then_parses_destination_with_effective_defaults(
    test_case: LoaderDestinationSelectionTestCase,
) -> None:
    discovered_inputs: DiscoveredProjectInputs = build_load_selection_inputs()
    fetch_loader: DiscoveredLoaderFunction = replace(
        discovered_inputs.loader_functions[0], destination=test_case.destination
    )
    discovered_inputs = replace(
        discovered_inputs,
        loader_functions=(fetch_loader, *discovered_inputs.loader_functions[1:]),
    )

    entries: tuple[SourceEntry, ...] = select_load_entries(
        discovered_inputs=discovered_inputs,
        select=("+raw_orders",),
        exclude=(),
        target_config=TargetConfig(database="target_db", schema="target_schema"),
        loader_default_database="connection_db",
        loader_default_schema="connection_schema",
    )

    assert (entries[0].database, entries[0].schema, entries[0].table) == test_case.expected_parts


@pytest.mark.parametrize(
    "test_case",
    (
        LoaderDefaultResolutionTestCase(
            description="connection namespace supplies non-duckdb loader defaults",
            adapter="postgres",
            connection={"database": "connection_db", "schema": "connection_schema"},
            expected_defaults=("connection_db", "connection_schema"),
        ),
        LoaderDefaultResolutionTestCase(
            description="project defaults come before the connection namespace",
            adapter="postgres",
            connection={"database": "connection_db", "schema": "connection_schema"},
            project_defaults=("defaults_db", "defaults_schema"),
            expected_defaults=("defaults_db", "defaults_schema"),
        ),
        LoaderDefaultResolutionTestCase(
            description="an inline connection is the default target on explicit-target adapters",
            adapter="snowflake",
            connection={"database": "CONNECTION_DB", "schema": "CONNECTION_SCHEMA"},
            expected_defaults=("CONNECTION_DB", "CONNECTION_SCHEMA"),
        ),
        LoaderDefaultResolutionTestCase(
            description="a named connection's location is never used",
            adapter="snowflake",
            connection={},
            connections={"warehouse": {"database": "NAMED_DB", "schema": "NAMED_SCHEMA"}},
            expected_defaults=(None, None),
        ),
        LoaderDefaultResolutionTestCase(
            description="explicit-target adapters take the project defaults",
            adapter="snowflake",
            connection={"database": "CONNECTION_DB", "schema": "CONNECTION_SCHEMA"},
            project_defaults=("DEFAULTS_DB", "DEFAULTS_SCHEMA"),
            expected_defaults=("DEFAULTS_DB", "DEFAULTS_SCHEMA"),
        ),
        LoaderDefaultResolutionTestCase(
            description="templated project defaults expand project vars as models do",
            adapter="postgres",
            connection={"database": "connection_db", "schema": "connection_schema"},
            project_defaults=("${warehouse_db}", "orders_${environment}"),
            project_vars={"warehouse_db": "analytics", "environment": "dev"},
            expected_defaults=("analytics", "orders_dev"),
        ),
        LoaderDefaultResolutionTestCase(
            description="command-line vars override project vars in templated defaults",
            adapter="postgres",
            connection={"database": "connection_db", "schema": "connection_schema"},
            project_defaults=(None, "orders_${environment}"),
            project_vars={"environment": "dev"},
            cli_vars={"environment": "ci"},
            expected_defaults=("connection_db", "orders_ci"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_connection_namespace_when_resolving_load_defaults_then_matches_compiled_project(
    test_case: LoaderDefaultResolutionTestCase,
) -> None:
    discovered_inputs: DiscoveredProjectInputs = DiscoveredProjectInputs(
        project_config=ProjectConfig(
            name="demo",
            adapter=test_case.adapter,
            connection=test_case.connection,
            connections=test_case.connections,
            defaults=DefaultsConfig(
                database=test_case.project_defaults[0], schema=test_case.project_defaults[1]
            ),
            vars=test_case.project_vars,
        ),
        local_config=LocalConfig(),
    )

    defaults: tuple[str | None, str | None] = resolve_loader_defaults(
        discovered_inputs=discovered_inputs,
        selected_target=None,
        target_config=None,
        cli_vars=test_case.cli_vars,
    )

    assert defaults == test_case.expected_defaults
