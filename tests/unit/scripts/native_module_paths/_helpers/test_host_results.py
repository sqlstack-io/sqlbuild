"""The executor result types in `sqlbuild-host` mirror the Python result dataclasses and enums."""

from __future__ import annotations

import pytest

from tests.unit.scripts.native_module_paths._helpers._test_types import HostResultMirrorCase
from tests.unit.scripts.native_module_paths._helpers.helpers import (
    HOST_RESULT_MODELS,
    mirrored_items,
    python_dataclass_fields,
    python_enum_values,
    rust_enum_values,
    rust_struct_fields,
)


@pytest.mark.parametrize(
    "test_case",
    [
        HostResultMirrorCase(
            description="every mirrored result enum and struct matches its Python values or fields",
            expected_mirror_count=33,
            expected_mismatches=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_host_result_types_when_compared_with_python_then_fields_and_values_agree(
    test_case: HostResultMirrorCase,
) -> None:
    source: str = HOST_RESULT_MODELS.read_text(encoding="utf-8")
    enums: dict[str, str] = mirrored_items(source=source, kind="enum")
    structs: dict[str, str] = mirrored_items(source=source, kind="struct")
    rust: set[tuple[str, tuple[str, ...]]] = {
        (path, rust_enum_values(source=source, name=name)) for path, name in enums.items()
    } | {(path, rust_struct_fields(source=source, name=name)) for path, name in structs.items()}
    python: set[tuple[str, tuple[str, ...]]] = {
        (path, python_enum_values(path=path)) for path in enums
    } | {(path, python_dataclass_fields(path=path)) for path in structs}

    mismatches: set[str] = {path for path, _ in rust ^ python}

    assert len(enums) + len(structs) == test_case.expected_mirror_count
    assert tuple(sorted(mismatches)) == test_case.expected_mismatches
