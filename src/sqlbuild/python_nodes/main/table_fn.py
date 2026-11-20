"""Authoring helper for SQLBuild-managed Python table functions."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any


def table_fn(**kwargs: object) -> Callable[[Callable[..., Any]], Callable[..., Any]]:
    """Mark a Python function as a SQLBuild table function for static project discovery."""

    del kwargs

    def _decorate(function: Callable[..., Any]) -> Callable[..., Any]:
        return function

    return _decorate
