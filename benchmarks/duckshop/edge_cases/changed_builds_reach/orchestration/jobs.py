"""Two scheduled jobs split the orders lineage and share int_orders; a third job has no trigger."""

from sqlbuild.orchestration import job, schedule

daily = schedule(cron="0 2 * * *", timezone="UTC")
staging = job(
    "staging_daily", description="Stage orders.", select="stg_orders int_orders", schedule=daily
)
finance = job(
    "finance_daily",
    description="Build revenue reports.",
    select="int_orders fct_revenue rpt_pnl",
    schedule=daily,
)
exports = job("adhoc_exports", description="Export orders on request.", select="orders_export")
