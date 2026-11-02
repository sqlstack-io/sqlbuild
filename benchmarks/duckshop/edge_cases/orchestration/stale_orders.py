from sqlbuild.orchestration import Job, Sensor, source_stale

orders_refresh = Job(
    "orders_refresh", description="Refresh orders from stale timestamp data.", select="orders"
)

on_stale_orders = Sensor(
    "on_stale_orders",
    description="Refresh orders once for each timestamp version older than one minute.",
    when=source_stale(select="source:raw_orders", after_seconds=60),
    run=orders_refresh,
    minimum_interval_seconds=1,
)
