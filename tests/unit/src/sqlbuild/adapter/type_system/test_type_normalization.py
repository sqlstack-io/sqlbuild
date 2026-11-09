from __future__ import annotations

import pytest

from sqlbuild.adapter.contract.models import NormalizedType
from sqlbuild.adapter.contract.types import TypeFamily
from sqlbuild.adapter.type_system.main.normalize_numeric_family import normalize_numeric_family
from sqlbuild.adapter.type_system.main.normalize_type import normalize_type
from sqlbuild.adapter.type_system.main.types_equal import types_equal
from tests.unit.src.sqlbuild.adapter.type_system._test_types import (
    NumericFamilyTestCase,
    TypeEqualityTestCase,
    TypeNormalizationTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        TypeNormalizationTestCase(
            description="normalizes snowflake number precision and scale",
            dialect="snowflake",
            raw_type="NUMBER(38,0)",
            expected_type=NormalizedType(
                normalized_name="DECIMAL(38,0)",
                family=TypeFamily.DECIMAL,
                precision=38,
                scale=0,
            ),
        ),
        TypeNormalizationTestCase(
            description="normalizes bigquery int64 to integer family",
            dialect="bigquery",
            raw_type="INT64",
            expected_type=NormalizedType(normalized_name="INT64", family=TypeFamily.INTEGER),
        ),
        TypeNormalizationTestCase(
            description="normalizes bigquery string to normalized string family",
            dialect="bigquery",
            raw_type="STRING",
            expected_type=NormalizedType(normalized_name="STRING", family=TypeFamily.STRING),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_type_string_when_normalizing_then_it_returns_expected_shape(
    test_case: TypeNormalizationTestCase,
) -> None:
    result: NormalizedType = normalize_type(type_sql=test_case.raw_type, dialect=test_case.dialect)

    assert result == test_case.expected_type


@pytest.mark.parametrize(
    "test_case",
    [
        TypeEqualityTestCase(
            description="treats bigquery integer aliases as equal",
            dialect="bigquery",
            left_type="INT64",
            right_type="INTEGER",
            expected_equal=True,
        ),
        TypeEqualityTestCase(
            description="treats bigquery boolean aliases as equal",
            dialect="bigquery",
            left_type="BOOL",
            right_type="BOOLEAN",
            expected_equal=True,
        ),
        TypeEqualityTestCase(
            description="treats snowflake text as the default varchar length",
            dialect="snowflake",
            left_type="VARCHAR(16777216)",
            right_type="TEXT",
            expected_equal=True,
        ),
        TypeEqualityTestCase(
            description="keeps explicit short snowflake varchar distinct from text",
            dialect="snowflake",
            left_type="VARCHAR(100)",
            right_type="TEXT",
            expected_equal=False,
        ),
        TypeEqualityTestCase(
            description="treats snowflake integer as number 38 scale 0",
            dialect="snowflake",
            left_type="INTEGER",
            right_type="NUMBER(38,0)",
            expected_equal=True,
        ),
        TypeEqualityTestCase(
            description="treats bare snowflake number as number 38 scale 0",
            dialect="snowflake",
            left_type="NUMBER",
            right_type="NUMBER(38,0)",
            expected_equal=True,
        ),
        TypeEqualityTestCase(
            description="treats snowflake timestamp alias as timestamp_ntz",
            dialect="snowflake",
            left_type="TIMESTAMP",
            right_type="TIMESTAMP_NTZ",
            expected_equal=True,
        ),
        TypeEqualityTestCase(
            description="treats a duckdb varchar length as the varchar duckdb stores",
            dialect="duckdb",
            left_type="VARCHAR",
            right_type="TEXT(5)",
            expected_equal=True,
        ),
        TypeEqualityTestCase(
            description="keeps duckdb varchar distinct from integer",
            dialect="duckdb",
            left_type="VARCHAR(5)",
            right_type="INTEGER",
            expected_equal=False,
        ),
        TypeEqualityTestCase(
            description="keeps snowflake varchar lengths distinct",
            dialect="snowflake",
            left_type="VARCHAR(5)",
            right_type="VARCHAR(10)",
            expected_equal=False,
        ),
        TypeEqualityTestCase(
            description="keeps snowflake timestamp time zone variants distinct",
            dialect="snowflake",
            left_type="TIMESTAMP_NTZ",
            right_type="TIMESTAMP_TZ",
            expected_equal=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_type_strings_when_comparing_then_it_returns_expected_equality(
    test_case: TypeEqualityTestCase,
) -> None:
    result: bool = types_equal(
        left=test_case.left_type,
        right=test_case.right_type,
        dialect=test_case.dialect,
    )

    assert result is test_case.expected_equal


@pytest.mark.parametrize(
    "test_case",
    [
        NumericFamilyTestCase(
            description="classifies bigquery integer alias as integer family",
            dialect="bigquery",
            raw_type="INTEGER",
            expected_family="integer",
        ),
        NumericFamilyTestCase(
            description="classifies snowflake number as decimal family",
            dialect="snowflake",
            raw_type="NUMBER(10,2)",
            expected_family="decimal",
        ),
        NumericFamilyTestCase(
            description="does not classify string as numeric",
            dialect="bigquery",
            raw_type="STRING",
            expected_family=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_type_string_when_resolving_numeric_family_then_it_returns_expected_family(
    test_case: NumericFamilyTestCase,
) -> None:
    result: str | None = normalize_numeric_family(
        type_sql=test_case.raw_type,
        dialect=test_case.dialect,
    )

    assert result == test_case.expected_family
