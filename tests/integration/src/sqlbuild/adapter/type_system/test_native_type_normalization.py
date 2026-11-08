"""Native type normalization: generated types, explicit shapes and Python's errors."""

from __future__ import annotations

import random
from collections.abc import Callable

import pytest

import sqlbuild._native as native_module
from sqlbuild.adapter.contract.models import NormalizedType
from sqlbuild.adapter.contract.types import TypeFamily
from sqlbuild.adapter.type_system._helpers.type_normalization import (
    normalize_type as cached_normalize_type,
)
from sqlbuild.adapter.type_system.main.normalize_type import normalize_type
from tests.integration.src.sqlbuild.adapter.type_system._test_types import (
    DeepTypeTestCase,
    GeneratedTypeTestCase,
    PublicNativeTypeTestCase,
    TypeNormalizationTestCase,
    UnknownDialectTypeTestCase,
)
from tests.integration.src.sqlbuild.adapter.type_system.helpers import (
    DIALECTS,
    TypeOutcome,
    dialect_outcomes,
    generated_type,
    is_normalized,
    logged_parse_error,
    type_outcome,
    type_outcomes,
)
from tests.integration.src.sqlbuild.compiler.helpers import (
    GoldenEntry,
    golden_differences,
    golden_entry,
    golden_name,
    read_golden,
)


@pytest.mark.parametrize(
    "test_case",
    [
        GeneratedTypeTestCase(
            description="seeded scalar, parameterized, nested, padded and invalid types",
            seed=20261008,
            count=1500,
            expected_minimum_normalized=16000,
            expected_minimum_parse_errors=5000,
            expected_unknown_dialect_errors=1500,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_generated_types_when_normalizing_then_every_known_dialect_answers(
    test_case: GeneratedTypeTestCase,
) -> None:
    rng: random.Random = random.Random(test_case.seed)
    type_strings: list[str] = [generated_type(rng=rng) for _ in range(test_case.count)]
    outcomes: list[TypeOutcome] = type_outcomes(type_strings=type_strings)
    normalized: list[TypeOutcome] = list(filter(is_normalized, outcomes))
    golden: list[GoldenEntry] = [
        golden_entry(
            "type", dialect_outcomes(outcomes[index * len(DIALECTS) : (index + 1) * len(DIALECTS)])
        )
        for index in range(len(type_strings))
    ]

    assert (
        golden_differences(
            read_golden(golden_name("types_generated", test_case.description)), golden
        )
        == []
    )
    assert (
        len(normalized) >= test_case.expected_minimum_normalized,
        sum(map(logged_parse_error, normalized)) >= test_case.expected_minimum_parse_errors,
        len(outcomes) - len(normalized),
    ) == (True, True, test_case.expected_unknown_dialect_errors), test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        TypeNormalizationTestCase(
            description="integer aliases under BigQuery",
            type_sql="INTEGER",
            dialect="bigquery",
            expected_type=NormalizedType(normalized_name="INT64", family=TypeFamily.INTEGER),
            expected_parse_error_logged=False,
        ),
        TypeNormalizationTestCase(
            description="snowflake number precision and scale defaults",
            type_sql="NUMBER",
            dialect="snowflake",
            expected_type=NormalizedType(
                normalized_name="DECIMAL(38,0)", family=TypeFamily.DECIMAL, precision=38, scale=0
            ),
            expected_parse_error_logged=False,
        ),
        TypeNormalizationTestCase(
            description="a decimal wider than the dialect holds keeps its precision",
            type_sql="DECIMAL(39, 2)",
            dialect="duckdb",
            expected_type=NormalizedType(
                normalized_name="DECIMAL(39,2)", family=TypeFamily.DECIMAL, precision=39, scale=2
            ),
            expected_parse_error_logged=False,
        ),
        TypeNormalizationTestCase(
            description="spaced decimal parameters",
            type_sql=" decimal( 10 , 2 ) ",
            dialect="duckdb",
            expected_type=NormalizedType(
                normalized_name="DECIMAL(10,2)", family=TypeFamily.DECIMAL, precision=10, scale=2
            ),
            expected_parse_error_logged=False,
        ),
        TypeNormalizationTestCase(
            description="unbounded snowflake text",
            type_sql="TEXT",
            dialect="snowflake",
            expected_type=NormalizedType(
                normalized_name="VARCHAR(16777216)", family=TypeFamily.STRING, length=16777216
            ),
            expected_parse_error_logged=False,
        ),
        TypeNormalizationTestCase(
            description="nested array of structs",
            type_sql="ARRAY<STRUCT<order_id INT, tags ARRAY<STRING>>>",
            dialect="bigquery",
            expected_type=NormalizedType(
                normalized_name="ARRAY<STRUCT<ORDER_IDINT64,TAGSARRAY<STRING>>>",
                family=TypeFamily.OTHER,
            ),
            expected_parse_error_logged=False,
        ),
        TypeNormalizationTestCase(
            description="map of varchar to decimal",
            type_sql="MAP(VARCHAR, DECIMAL(18, 4))",
            dialect="duckdb",
            expected_type=NormalizedType(
                normalized_name="MAP(TEXT,DECIMAL(18,4))", family=TypeFamily.OTHER
            ),
            expected_parse_error_logged=False,
        ),
        TypeNormalizationTestCase(
            description="quoted custom type",
            type_sql='"Order Status"',
            dialect="postgres",
            expected_type=NormalizedType(normalized_name="ORDERSTATUS", family=TypeFamily.OTHER),
            expected_parse_error_logged=False,
        ),
        TypeNormalizationTestCase(
            description="timestamp aliases",
            type_sql="timestamp_ntz(9)",
            dialect="snowflake",
            expected_type=NormalizedType(
                normalized_name="TIMESTAMP_NTZ", family=TypeFamily.TIMESTAMP
            ),
            expected_parse_error_logged=False,
        ),
        TypeNormalizationTestCase(
            description="invalid type falls back after a logged parse error",
            type_sql="order status",
            dialect="generic",
            expected_type=NormalizedType(normalized_name="ORDERSTATUS", family=TypeFamily.OTHER),
            expected_parse_error_logged=True,
        ),
        TypeNormalizationTestCase(
            description="parameters beyond i64 stay Python integers",
            type_sql="VARCHAR(99999999999999999999)",
            dialect="snowflake",
            expected_type=NormalizedType(
                normalized_name="VARCHAR(99999999999999999999)",
                family=TypeFamily.STRING,
                length=99999999999999999999,
            ),
            expected_parse_error_logged=True,
        ),
        TypeNormalizationTestCase(
            description="array suffixes past the old bracket cap",
            type_sql="INT" + "[]" * 33 + "",
            dialect="duckdb",
            expected_type=NormalizedType(
                normalized_name="INT" + "[]" * 33, family=TypeFamily.OTHER
            ),
            expected_parse_error_logged=False,
        ),
        TypeNormalizationTestCase(
            description="non-ASCII text upper-cases as Python does",
            type_sql="caf\u00e9",
            dialect="generic",
            expected_type=NormalizedType(normalized_name="CAF\u00c9", family=TypeFamily.OTHER),
            expected_parse_error_logged=False,
        ),
        TypeNormalizationTestCase(
            description="non-ASCII text folds sharp s as Python does",
            type_sql="stra\u00dfe",
            dialect="snowflake",
            expected_type=NormalizedType(normalized_name="STRASSE", family=TypeFamily.OTHER),
            expected_parse_error_logged=False,
        ),
        TypeNormalizationTestCase(
            description="Unicode decimal digits are Python integers",
            type_sql="VARCHAR(\u0663)",
            dialect="snowflake",
            expected_type=NormalizedType(
                normalized_name="VARCHAR(3)", family=TypeFamily.STRING, length=3
            ),
            expected_parse_error_logged=True,
        ),
        TypeNormalizationTestCase(
            description="a dialect outside the old native build",
            type_sql="INT",
            dialect="mysql",
            expected_type=NormalizedType(normalized_name="INT", family=TypeFamily.INTEGER),
            expected_parse_error_logged=False,
        ),
        TypeNormalizationTestCase(
            description="a dialect-specific type outside the old native build",
            type_sql="UInt8",
            dialect="clickhouse",
            expected_type=NormalizedType(normalized_name="UINT8", family=TypeFamily.OTHER),
            expected_parse_error_logged=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_type_when_normalizing_then_python_shape_is_returned(
    test_case: TypeNormalizationTestCase,
) -> None:
    outcome: TypeOutcome = type_outcome(type_sql=test_case.type_sql, dialect=test_case.dialect)

    assert (outcome.normalized, logged_parse_error(outcome)) == (
        test_case.expected_type,
        test_case.expected_parse_error_logged,
    ), test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        UnknownDialectTypeTestCase(
            description="a dialect name SQLBuild does not know",
            dialect="motherduck",
            expected_error="ValueError: Unknown dialect: motherduck",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unknown_dialect_when_normalizing_then_python_error_is_raised(
    test_case: UnknownDialectTypeTestCase,
) -> None:
    outcome: TypeOutcome = type_outcome(type_sql="INT", dialect=test_case.dialect)

    assert outcome.normalized == test_case.expected_error, test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        DeepTypeTestCase(
            description="5,000 array suffixes, a depth the Python wheel normalized",
            type_sql="INT" + "[]" * 5_000,
            expected_family=TypeFamily.OTHER,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_deeply_nested_type_when_normalizing_then_python_shape_is_returned(
    test_case: DeepTypeTestCase,
) -> None:
    outcome: TypeOutcome = type_outcome(type_sql=test_case.type_sql, dialect="duckdb")

    assert outcome.normalized == NormalizedType(
        normalized_name=test_case.type_sql, family=test_case.expected_family
    ), test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        PublicNativeTypeTestCase(
            description="the public entry point asks native first",
            dialect="snowflake",
            type_strings=("NUMBER(38, 0)", "VARCHAR", "TIMESTAMP_NTZ(9)"),
            expected_native_calls=(
                ("NUMBER(38, 0)", "snowflake"),
                ("VARCHAR", "snowflake"),
                ("TIMESTAMP_NTZ(9)", "snowflake"),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_type_strings_when_normalizing_publicly_then_native_answers(
    test_case: PublicNativeTypeTestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    calls: list[tuple[str, str]] = []
    answers: list[object] = []
    native_normalize: Callable[[str, str], object] = native_module.normalize_type

    def spy(type_sql: str, dialect: str) -> object:
        calls.append((type_sql, dialect))
        answers.append(native_normalize(type_sql, dialect))
        return answers[-1]

    monkeypatch.setattr(native_module, "normalize_type", spy)
    cached_normalize_type.cache_clear()
    normalized: list[NormalizedType] = [
        normalize_type(type_sql=type_sql, dialect=test_case.dialect)
        for type_sql in test_case.type_strings
    ]
    cached_normalize_type.cache_clear()

    assert (tuple(calls), len(answers), len(normalized)) == (
        test_case.expected_native_calls,
        len(test_case.type_strings),
        len(test_case.type_strings),
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
