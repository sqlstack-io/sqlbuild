"""Postgres differential failures."""


class PgDifferentialError(RuntimeError):
    """Raised when captures cannot be read or the harness cannot run."""
