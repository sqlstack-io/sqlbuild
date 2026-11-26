"""The loading facts in a loader's SQLLoad table options, as the planner and executor read them."""

from collections.abc import Mapping

from sqlbuild._native import SourceWriteStrategy

_SQLLOAD_REPLACE: str = "replace"


def bridged_write_strategy(settings: Mapping[str, object]) -> SourceWriteStrategy | None:
    """The write strategy of `settings`, SQLLoad's `replace` read as `table`."""
    value: object = settings.get("write_strategy")
    if value == _SQLLOAD_REPLACE:
        return SourceWriteStrategy.TABLE
    if value == SourceWriteStrategy.TABLE.value or not isinstance(value, str):
        return None
    try:
        return SourceWriteStrategy(value)
    except ValueError:
        return None


def bridged_text(*, settings: Mapping[str, object], key: str) -> str | None:
    """The text option `key` of `settings`, if it is text."""
    value: object = settings.get(key)
    return value if isinstance(value, str) else None


def bridged_names(*, settings: Mapping[str, object], key: str) -> tuple[str, ...]:
    """The column names option `key` of `settings`: one name or a list of them."""
    value: object = settings.get(key)
    if isinstance(value, str):
        return (value,)
    if isinstance(value, list | tuple):
        return tuple(str(name) for name in value)
    return ()
