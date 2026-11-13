"""Two scheduled jobs each own one staging model; customer_orders reads both, in crm_daily."""

from sqlbuild.orchestration import job, schedule

daily = schedule(cron="0 2 * * *", timezone="UTC")
crm = job(
    "crm_daily",
    description="Refresh customers.",
    select="stg_customers customer_orders",
    schedule=daily,
)
sales = job(
    "sales_daily",
    description="Refresh sales.",
    select="stg_orders customer_sales",
    schedule=daily,
)
