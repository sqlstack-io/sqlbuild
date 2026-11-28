"""Refusing a cursor window that a loader the run loads cannot apply."""

from __future__ import annotations

from collections.abc import Iterable

from sqlbuild.errors.contracts.exceptions import ExecutorInputError


def refuse_loaders_without_cursor(
    *, loaders: Iterable[tuple[str, str | None]], window: Iterable[object | None]
) -> None:
    """Refuse a cursor `window` when one of `loaders`, as (name, cursor_column), has no cursor."""

    if all(value is None for value in window):
        return
    names: list[str] = sorted({name for name, cursor_column in loaders if cursor_column is None})
    if not names:
        return
    quoted: str = ", ".join(f"'{name}'" for name in names)
    raise ExecutorInputError(
        f"--start-cursor/--end-cursor set a cursor window, but loader {quoted} "
        "declares no cursor_column, so the window would not apply to it",
        help="set cursor_column on the @loader, or leave its source out with --exclude",
    )
