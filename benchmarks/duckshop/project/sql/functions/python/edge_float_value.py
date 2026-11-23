from sqlbuild.functions import udf


@udf(
    arguments={"amount": "NUMBER(38, 0)"},
    returns="FLOAT",
    runtime_version="3.11",
    tags=["edge_case"],
)
def main(amount: int | None) -> int | None:
    """The amount handed back as a Python int to a FLOAT result; past 2^53 it loses digits."""
    return amount
