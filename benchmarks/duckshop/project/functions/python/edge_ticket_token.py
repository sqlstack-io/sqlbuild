import uuid

from sqlbuild.functions import udf


@udf(
    arguments={"seed": "VARCHAR"},
    returns="VARCHAR",
    runtime_version="3.11",
    tags=["edge_case"],
)
def main(seed: str | None) -> str:
    """A fresh random ticket token on every call, whatever the seed: Python UDFs are volatile."""
    del seed
    return uuid.uuid4().hex
