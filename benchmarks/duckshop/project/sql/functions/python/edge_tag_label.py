from sqlbuild.functions import udf


@udf(
    arguments={"tag": "VARCHAR"},
    returns="VARCHAR",
    runtime_version="3.11",
    tags=["edge_case"],
)
def main(tag: str | None) -> str:
    """A basket tag as a display label; 'Untagged' for a missing tag.

    The function is not declared STRICT, so Snowflake calls the handler for NULL input too and
    returns its answer, not NULL.
    """
    if tag is None or not tag.strip():
        return "Untagged"
    return tag.strip().capitalize()
