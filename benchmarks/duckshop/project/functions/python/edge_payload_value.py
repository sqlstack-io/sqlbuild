from datetime import UTC, datetime
from decimal import Decimal

from sqlbuild.functions import udf


@udf(
    arguments={"kind": "VARCHAR"},
    returns="VARIANT",
    runtime_version="3.11",
    tags=["edge_case"],
)
def main(kind: str | None) -> object:
    """A partner payload value of the named kind, as Python hands it to a VARIANT.

    None becomes JSON null, not SQL NULL. Datetimes, Decimals and integers past 2^53 cross as
    Snowflake converts them.
    """
    values: dict[str, object] = {
        "nothing": None,
        "zoned_time": datetime(2026, 3, 1, 9, 30, tzinfo=UTC),
        "naive_time": datetime(2026, 3, 1, 9, 30),
        "naive_time_fraction": datetime(2026, 3, 1, 9, 30, 0, 250000),
        "exponent_decimal": Decimal("1E+3"),
        "plain_decimal": Decimal("1.50"),
        "wide_integer": 2**60 + 1,
        "nested": {"b": [1, None], "a": True},
    }
    return values.get(kind or "", kind)
