"""Time expression macros."""

from sqlbuild.compiler.compile.models import MacroContext


def timestamp_trunc(ctx: MacroContext, grain: str, expression: str) -> str:
    """Render timestamp truncation for the active adapter."""

    if ctx.adapter_name == "bigquery":
        return f"TIMESTAMP_TRUNC({expression}, {grain.upper()})"
    return f"DATE_TRUNC('{grain}', {expression})"
