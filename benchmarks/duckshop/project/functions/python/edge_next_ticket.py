import uuid

from sqlbuild.functions import udf


@udf(
    arguments={},
    returns="NUMBER(38, 0)",
    runtime_version="3.11",
    tags=["edge_case"],
)
def main() -> int:
    """A fresh random positive ticket number on every call, from no arguments."""
    return uuid.uuid4().int % 10**15 + 1
