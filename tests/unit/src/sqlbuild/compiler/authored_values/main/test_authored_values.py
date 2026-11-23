from __future__ import annotations

import pytest

from tests.unit.src.sqlbuild.compiler.authored_values.main._test_types import (
    AuthoredValueErrorTestCase,
    AuthoredValueTestCase,
)
from tests.unit.src.sqlbuild.compiler.authored_values.main.helpers import (
    AuthoredValueTestError,
    parse_named_bool_default_false,
    parse_named_bool_default_none,
    parse_named_string,
)


@pytest.mark.parametrize(
    "test_case",
    [
        AuthoredValueTestCase(
            description="named string defaults to none",
            parse=parse_named_string,
            raw_value=None,
            expected_value=None,
        ),
        AuthoredValueTestCase(
            description="named bool uses false default",
            parse=parse_named_bool_default_false,
            raw_value=None,
            expected_value=False,
        ),
        AuthoredValueTestCase(
            description="named bool uses none default",
            parse=parse_named_bool_default_none,
            raw_value=None,
            expected_value=None,
        ),
        AuthoredValueTestCase(
            description="named bool returns true over default",
            parse=parse_named_bool_default_false,
            raw_value=True,
            expected_value=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_authored_value_when_parsing_then_returns_expected_value(
    test_case: AuthoredValueTestCase,
) -> None:
    assert test_case.parse(test_case.raw_value) == test_case.expected_value


@pytest.mark.parametrize(
    "test_case",
    [
        AuthoredValueErrorTestCase(
            description="named string rejects non-string value",
            parse=parse_named_string,
            raw_value=3,
            expected_message="sql/models/orders.yml model 'orders' 'setting' must be a non-empty string",
        ),
        AuthoredValueErrorTestCase(
            description="named bool rejects integer value",
            parse=parse_named_bool_default_false,
            raw_value=1,
            expected_message="sql/models/orders.yml model 'orders' 'setting' must be a boolean",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_authored_value_when_parsing_then_raises_injected_error(
    test_case: AuthoredValueErrorTestCase,
) -> None:
    with pytest.raises(AuthoredValueTestError) as error:
        test_case.parse(test_case.raw_value)

    assert str(error.value) == test_case.expected_message
