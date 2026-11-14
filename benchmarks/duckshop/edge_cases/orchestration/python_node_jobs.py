"""Jobs whose selections are Python nodes, which `--select job:<name>` must select as published."""

from sqlbuild.orchestration import job

customer_counts = job(
    "customer_counts",
    description="Count customers.",
    select="task:count_customers asset:customers_extract",
)
order_counts = job("order_counts", description="Count orders.", select="+task:count_orders")
