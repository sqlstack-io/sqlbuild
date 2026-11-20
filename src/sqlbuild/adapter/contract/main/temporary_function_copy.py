"""Shared session TEMPORARY copies of a function's CREATE statements."""

from __future__ import annotations

from sqlbuild.adapter.contract.models import TemporaryFunction


def temporary_function_copy(*, target: str, statements: tuple[str, ...]) -> TemporaryFunction:
    """The `CREATE OR REPLACE FUNCTION` statements of `target` as TEMPORARY functions."""

    return TemporaryFunction(
        target=target,
        statements=tuple(
            statement.replace(
                "CREATE OR REPLACE FUNCTION ", "CREATE OR REPLACE TEMPORARY FUNCTION ", 1
            )
            for statement in statements
        ),
    )
