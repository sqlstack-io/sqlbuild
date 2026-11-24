from sqlbuild.functions import udf


@udf(
    arguments={"order_status": "STRING"},
    returns="BOOLEAN",
    runtime_version="3.11",
)
def main(order_status: str | None) -> bool:
    """Test function main."""
    return order_status == "completed"
