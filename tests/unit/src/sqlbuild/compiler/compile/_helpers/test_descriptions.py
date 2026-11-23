"""Every discovered input kind is either checked by P010 or explicitly exempt."""

from __future__ import annotations

from dataclasses import fields

import pytest

from sqlbuild._native import DiscoveredProjectInputs
from tests.unit.src.sqlbuild.compiler.compile._helpers._test_types import (
    DescriptionInputClassificationCase,
)

DESCRIPTION_EXEMPT_INPUTS: dict[str, str] = {
    "project_config": "project configuration, not a named resource",
    "local_config": "local configuration, not a named resource",
    "project_dir": "project location, not a named resource",
    "adapter_file": "project adapter override, not a named resource",
    "enum_files": "typed value declarations used inside SQL, not graph resources",
    "constant_files": "typed value declarations used inside SQL, not graph resources",
    "macro_files": "SQL helpers expanded inline, not graph resources",
    "model_schema_files": "shared column schemas; models that use one are checked",
    "schema_files": "seed YAML declarations; each declared seed is checked",
    "test_files": "SQL unit tests are named by their subject and expected behaviour",
    "audit_files": "audits are assertions named by what they check",
    "audit_factories": "generate audits, which are assertions",
    "discover_functions": "list a set's items, not graph resources",
    "for_each_assets": "templates of asset nodes; each item's node is checked as an asset",
    "materialization_files": "materialization strategies, not graph resources",
    "event_exporters": "runtime event outputs, not graph resources",
    "command_output_sinks": "runtime command outputs, not graph resources",
    "native_session": "native parse state kept for later stages, not a project input",
}
DESCRIPTION_REQUIRED_INPUT_KINDS: dict[str, str] = {
    "model_files": "model",
    "scenario_files": "scenario",
    "seed_files": "seed",
    "source_files": "source",
    "sql_function_files": "function",
    "python_function_files": "function",
    "sql_hook_files": "hook",
    "hook_functions": "hook",
    "loader_functions": "loader",
    "asset_functions": "asset",
    "check_functions": "check",
    "providers": "provider",
}


@pytest.mark.parametrize(
    "test_case",
    [
        DescriptionInputClassificationCase(
            description="every discovered input field is required or exempt",
            required=frozenset(DESCRIPTION_REQUIRED_INPUT_KINDS),
            exempt=frozenset(DESCRIPTION_EXEMPT_INPUTS),
            expected_fields=frozenset(item.name for item in fields(DiscoveredProjectInputs)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_discovered_input_fields_when_classifying_then_every_field_is_checked_or_exempt(
    test_case: DescriptionInputClassificationCase,
) -> None:
    assert not test_case.required & test_case.exempt
    assert test_case.required | test_case.exempt == test_case.expected_fields


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
