"""Snowflake column type strings from INFORMATION_SCHEMA fields and SHOW COLUMNS metadata."""

from __future__ import annotations

import json
from functools import lru_cache

from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapters.snowflake.constants import NUMBER_TYPE_NAME, TEXT_TYPE_NAMES

_SHOW_TYPE_TO_INFORMATION_SCHEMA_TYPE: dict[str, str] = {
    "FIXED": NUMBER_TYPE_NAME,
    "REAL": "FLOAT",
}
_SHOW_TYPE_CACHE_SIZE: int = 4096
_INVALID_SHOW_TYPE_MESSAGE: str = "Snowflake SHOW COLUMNS returned invalid type metadata"


def information_schema_type(
    *,
    data_type: str,
    numeric_precision: object,
    numeric_scale: object,
    character_maximum_length: object,
) -> str:
    """Render the planner type string for one INFORMATION_SCHEMA.COLUMNS row."""

    normalized_type: str = data_type.upper()
    if (
        normalized_type == NUMBER_TYPE_NAME
        and isinstance(numeric_precision, int)
        and isinstance(numeric_scale, int)
    ):
        return f"{NUMBER_TYPE_NAME}({numeric_precision},{numeric_scale})"
    if normalized_type in TEXT_TYPE_NAMES and isinstance(character_maximum_length, int):
        return f"VARCHAR({character_maximum_length})"
    return normalized_type


def show_columns_type(raw_data_type: object) -> str:
    """Render the same type string as INFORMATION_SCHEMA for one SHOW COLUMNS data_type JSON."""

    return _show_type(str(raw_data_type))


@lru_cache(maxsize=_SHOW_TYPE_CACHE_SIZE)
def _show_type(raw_data_type: str) -> str:
    """`show_columns_type` of one JSON text; a schema repeats a few hundred texts in 100K+ rows."""

    try:
        decoded: object = json.loads(raw_data_type)
    except (json.JSONDecodeError, TypeError) as error:
        raise AdapterUserError(message=_INVALID_SHOW_TYPE_MESSAGE) from error
    if not isinstance(decoded, dict) or not isinstance(decoded.get("type"), str):
        raise AdapterUserError(message=_INVALID_SHOW_TYPE_MESSAGE)
    raw_name: str = str(decoded["type"]).upper()
    return information_schema_type(
        data_type=_SHOW_TYPE_TO_INFORMATION_SCHEMA_TYPE.get(raw_name, raw_name),
        numeric_precision=_metadata_integer(decoded.get("precision")),
        numeric_scale=_metadata_integer(decoded.get("scale")),
        character_maximum_length=_metadata_integer(decoded.get("length")),
    )


def _metadata_integer(value: object) -> int | None:
    if isinstance(value, bool) or not isinstance(value, int):
        return None
    return value
