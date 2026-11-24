"""Generate typed extractions from Snowflake semi-structured columns."""


def variant_fields(source: str, fields: dict[str, str]) -> str:
    """Typed `:` path extraction for each `alias: "path TYPE"` entry, in the given order."""
    items = []
    for alias, spec in fields.items():
        path, _, sql_type = spec.rpartition(" ")
        items.append(f"{source}:{path}::{sql_type} AS {alias}")
    return ",\n  ".join(items)
