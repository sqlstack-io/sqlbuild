"""Errors a loader's run on SQLLoad's engine raises."""

from __future__ import annotations


class LoaderSkippedError(Exception):
    """The loader returned `ctx.skip(...)`: its table stops before anything loads."""
