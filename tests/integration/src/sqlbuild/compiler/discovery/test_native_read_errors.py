"""Native discovery reports unreadable authored files with Python's own read errors."""

from __future__ import annotations

import random
from pathlib import Path

import pytest

import sqlbuild._native as _native
from tests.integration.src.sqlbuild.compiler.discovery._test_types import (
    NativeOsErrorTestCase,
    ReadErrorOracleTestCase,
    UnreadableFileTestCase,
)
from tests.integration.src.sqlbuild.compiler.discovery.helpers import (
    compile_failure,
    native_read_outcomes,
    python_read_outcomes,
    random_undecodable_bytes,
    write_project,
)
from tests.integration.src.sqlbuild.compiler.helpers import mismatches

_STAGING: bytes = b'MODEL (description "Orders");\nSELECT 1 AS order_id'


@pytest.mark.parametrize(
    "test_case",
    [
        ReadErrorOracleTestCase(
            description="seeded cut, corrupted and invalid UTF-8 sequences",
            seed=97,
            case_count=3000,
            expected_minimum_failures=1500,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_undecodable_files_when_reading_natively_then_errors_equal_read_text_errors(
    test_case: ReadErrorOracleTestCase, tmp_path: Path
) -> None:
    rng: random.Random = random.Random(test_case.seed)
    files: tuple[tuple[str, bytes], ...] = tuple(
        (f"sql/sources/file_{index}.yml", random_undecodable_bytes(rng=rng))
        for index in range(test_case.case_count)
    )
    write_project(project_dir=tmp_path, files=files)
    relative_paths: list[str] = [path for path, _data in files]
    python: list[object] = python_read_outcomes(project_dir=tmp_path, relative_paths=relative_paths)

    native: list[object] = native_read_outcomes(project_dir=tmp_path, relative_paths=relative_paths)

    failures: int = sum(outcome != ("NoneType", "None") for outcome in python)
    assert (
        mismatches(inputs=list(relative_paths), expected=python, actual=native),
        failures >= test_case.expected_minimum_failures,
    ) == ([], True), test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        UnreadableFileTestCase(
            description="an undecodable model",
            relative_path="sql/models/orders.sql",
            data=_STAGING + b"\xff",
            expected_error="UnicodeDecodeError",
            expected_message=(
                "'utf-8' codec can't decode byte 0xff in position 50: invalid start byte"
            ),
        ),
        UnreadableFileTestCase(
            description="a SQL test cut inside a sequence",
            relative_path="tests/unit/orders.sql",
            data=b'TEST (name "keeps");\nSELECT \xe2\x82',
            expected_error="UnicodeDecodeError",
            expected_message=(
                "'utf-8' codec can't decode bytes in position 28-29: unexpected end of data"
            ),
        ),
        UnreadableFileTestCase(
            description="a scenario with an invalid continuation byte",
            relative_path="tests/scenarios/orders.sql",
            data=b'SCENARIO (description "x");\nSELECT \xe2\x82x',
            expected_error="UnicodeDecodeError",
            expected_message=(
                "'utf-8' codec can't decode bytes in position 35-36: invalid continuation byte"
            ),
        ),
        UnreadableFileTestCase(
            description="an undecodable source file",
            relative_path="sql/sources/raw.yml",
            data=b"sources: []\n# \xc0\n",
            expected_error="UnicodeDecodeError",
            expected_message=(
                "'utf-8' codec can't decode byte 0xc0 in position 14: invalid start byte"
            ),
        ),
        UnreadableFileTestCase(
            description="a directory named like a schema file",
            relative_path="sql/models/marts/schema.yml/notes.txt",
            data=b"",
            expected_error="IsADirectoryError",
            expected_message="[Errno 21] Is a directory: '{project}/sql/models/marts/schema.yml'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unreadable_file_when_discovering_then_python_read_error_is_raised(
    test_case: UnreadableFileTestCase, tmp_path: Path
) -> None:
    write_project(
        project_dir=tmp_path,
        files=(("sql/models/staging.sql", _STAGING), (test_case.relative_path, test_case.data)),
    )

    failure: tuple[str, str] = compile_failure(project_dir=tmp_path)

    assert failure == (
        test_case.expected_error,
        test_case.expected_message.format(project=tmp_path),
    ), test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        NativeOsErrorTestCase(
            description="a refused read is Python's errno error naming the file",
            payload=(13, None, "Permission denied (os error 13)"),
            expected_type="PermissionError",
            expected_fields=(13, "Permission denied", "orders.sql"),
        ),
        NativeOsErrorTestCase(
            description="a Windows listing error carries its Win32 code for Python to map",
            payload=(None, 5, "Access is denied"),
            expected_type="OSError",
            expected_fields=(None, "Access is denied", "orders.sql"),
        ),
        NativeOsErrorTestCase(
            description="an error without a code keeps the operating system message",
            payload=(None, None, "stream did not contain valid UTF-8"),
            expected_type="OSError",
            expected_fields=(None, None, None),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_os_read_failure_when_building_error_then_python_error_matches_platform(
    test_case: NativeOsErrorTestCase,
) -> None:
    error: Exception = _native._oracle_read_error(*test_case.payload, file_path=Path("orders.sql"))

    assert (
        type(error).__name__,
        (
            getattr(error, "errno", None),
            getattr(error, "strerror", None),
            getattr(error, "filename", None),
        ),
    ) == (test_case.expected_type, test_case.expected_fields), test_case.description


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
