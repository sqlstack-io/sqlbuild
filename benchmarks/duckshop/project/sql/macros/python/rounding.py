"""Adapter-aware rounding."""


def round_half_even(ctx, expression: str, places: int = 2) -> str:
    """Banker's rounding of a decimal expression, in the adapter's native form when it has one."""
    if ctx.adapter_name == "snowflake":
        return f"ROUND({expression}, {places}, 'HALF_TO_EVEN')"
    return f"ROUND_EVEN({expression}, {places})"
