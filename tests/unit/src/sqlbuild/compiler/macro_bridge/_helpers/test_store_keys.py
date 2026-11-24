from __future__ import annotations

from dataclasses import replace
from pathlib import Path

import pytest

from sqlbuild._native import (
    context_store_token,
    macro_store_token,
)
from tests.unit.src.sqlbuild.compiler.macro_bridge._helpers._test_types import (
    ContextStoreTokenTestCase,
    ContextValueTestCase,
    MacroStoreTokenTestCase,
)
from tests.unit.src.sqlbuild.compiler.macro_bridge._helpers.helpers import (
    BASE_CONTEXT,
    constant_declarations,
    deeply_nested_list,
    loaded_macro,
    with_vars,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ContextStoreTokenTestCase(
            description="equal values in separate objects",
            left=with_vars(),
            right=with_vars(),
            expected_equal=True,
            left_declarations=constant_declarations(1),
            right_declarations=constant_declarations(1),
        ),
        ContextStoreTokenTestCase(
            description="var value changed",
            left=with_vars(),
            right=with_vars(region="south"),
            expected_equal=False,
        ),
        ContextStoreTokenTestCase(
            description="integer and float var values",
            left=with_vars(region=1),
            right=with_vars(region=1.0),
            expected_equal=False,
        ),
        ContextStoreTokenTestCase(
            description="boolean and integer var values",
            left=with_vars(region=1),
            right=with_vars(region=True),
            expected_equal=False,
        ),
        ContextStoreTokenTestCase(
            description="var order changed",
            left=with_vars(first=1, second=2),
            right=replace(BASE_CONTEXT, vars={**BASE_CONTEXT.vars, "second": 2, "first": 1}),
            expected_equal=False,
        ),
        ContextStoreTokenTestCase(
            description="target changed",
            left=with_vars(),
            right=replace(BASE_CONTEXT, target_name="prod"),
            expected_equal=False,
        ),
        ContextStoreTokenTestCase(
            description="visible constant value changed",
            left=with_vars(),
            right=with_vars(),
            expected_equal=False,
            left_declarations=constant_declarations(1),
            right_declarations=constant_declarations(2),
        ),
        ContextStoreTokenTestCase(
            description="declarations missing on one side",
            left=with_vars(),
            right=with_vars(),
            expected_equal=False,
            left_declarations=constant_declarations(1),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_two_contexts_when_naming_their_store_class_then_only_equal_values_match(
    test_case: ContextStoreTokenTestCase,
) -> None:
    left: str | None = context_store_token(
        macro_context=test_case.left, declarations=test_case.left_declarations
    )
    right: str | None = context_store_token(
        macro_context=test_case.right, declarations=test_case.right_declarations
    )

    assert left is not None
    assert (left == right) is test_case.expected_equal


@pytest.mark.parametrize(
    "test_case",
    [
        ContextValueTestCase(
            description="dates, decimals and floats", context=with_vars(), expected_stored=True
        ),
        ContextValueTestCase(
            description="object var", context=with_vars(region=object()), expected_stored=False
        ),
        ContextValueTestCase(
            description="nested object var",
            context=with_vars(region={"key": [object()]}),
            expected_stored=False,
        ),
        ContextValueTestCase(
            description="lone surrogate var",
            context=with_vars(region="\udcff"),
            expected_stored=False,
        ),
        ContextValueTestCase(
            description="var nested beyond the recursion limit",
            context=with_vars(region=deeply_nested_list(depth=100_000)),
            expected_stored=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_context_value_when_naming_store_class_then_only_stable_values_are_stored(
    test_case: ContextValueTestCase,
) -> None:
    token: str | None = context_store_token(macro_context=test_case.context, declarations=None)

    assert (token is not None) is test_case.expected_stored


@pytest.mark.parametrize(
    "test_case",
    [
        MacroStoreTokenTestCase(
            description="same file and source",
            relative_path=Path("sql/macros/python/common.py"),
            raw_source="def cents(column): ...\n",
            expected_equal=True,
        ),
        MacroStoreTokenTestCase(
            description="source edited",
            relative_path=Path("sql/macros/python/common.py"),
            raw_source="def cents(column): return column\n",
            expected_equal=False,
        ),
        MacroStoreTokenTestCase(
            description="same source in a scoped file",
            relative_path=Path("sql/models/south/_sqlbuild/_macros/python/common.py"),
            raw_source="def cents(column): ...\n",
            expected_equal=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loaded_macro_change_when_naming_it_then_only_the_same_file_and_source_match(
    test_case: MacroStoreTokenTestCase,
) -> None:
    original: str | None = macro_store_token(
        loaded=loaded_macro(
            relative_path=Path("sql/macros/python/common.py"), raw_source="def cents(column): ...\n"
        ),
        identity=None,
    )

    changed: str | None = macro_store_token(
        loaded=loaded_macro(relative_path=test_case.relative_path, raw_source=test_case.raw_source),
        identity=None,
    )

    assert (original == changed) is test_case.expected_equal
