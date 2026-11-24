"""Datetime portability macros."""


def timestamp_trunc(ctx, grain: str, expr: str) -> str:
    if ctx.adapter_name == "bigquery":
        return f"TIMESTAMP_TRUNC({expr}, {grain.upper()})"
    return f"DATE_TRUNC('{grain}', {expr})"
