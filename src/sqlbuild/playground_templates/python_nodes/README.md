# SQLBuild Python Nodes Playground

This project demonstrates first-class Python nodes in a small local DuckDB project.

Try:

```bash
sqb plan --select +fact_orders --select +orders_export
sqb build --select +fact_orders --select +orders_export
sqb check --select check_orders_export
```

The project includes an asset feeding a loader, a model feeding Python assets through
`ctx.relation(model(...))`, soft skip fan-in, `materialized=False`, and a Python check.
