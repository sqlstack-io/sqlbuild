from __future__ import annotations

from pathlib import Path

from sqlbuild._native import optional_named_bool, optional_named_string

AUTHORED_VALUES_FILE_PATH: Path = Path("sql/models/orders.yml")
AUTHORED_VALUES_LABEL: str = "model 'orders'"


class AuthoredValueTestError(Exception):
    """Error class injected into authored value parsers under test."""


def parse_named_string(raw_value: object | None) -> object:
    return optional_named_string(
        raw_value=raw_value,
        file_path=AUTHORED_VALUES_FILE_PATH,
        label=AUTHORED_VALUES_LABEL,
        key="setting",
        error_class=AuthoredValueTestError,
    )


def parse_named_bool_default_false(raw_value: object | None) -> object:
    return optional_named_bool(
        raw_value=raw_value,
        file_path=AUTHORED_VALUES_FILE_PATH,
        label=AUTHORED_VALUES_LABEL,
        key="setting",
        error_class=AuthoredValueTestError,
        default=False,
    )


def parse_named_bool_default_none(raw_value: object | None) -> object:
    return optional_named_bool(
        raw_value=raw_value,
        file_path=AUTHORED_VALUES_FILE_PATH,
        label=AUTHORED_VALUES_LABEL,
        key="setting",
        error_class=AuthoredValueTestError,
        default=None,
    )
