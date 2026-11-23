# Landing page examples

The projects that produced the real `sqb` output on the landing page, so it can be regenerated when
the CLI changes. Build output, logs and DuckDB files are not kept; every project runs locally on
DuckDB.

| Project | Landing page |
|---------|--------------|
| `waffle-shop/` | Steps 01 to 07. It is `sqb playground waffle-shop` plus the custom Rule in `rules/layers.py` and its test, the scoped enum and macros under `sql/models/marts/_sqlbuild/`, and the renamed `daily_order_rollup` model. |
| `tidy-shop/` | Step 08, the janitor. |
| `hero-shop/` | The hero: a model moved and renamed. |

## Step 01: compile errors

Step 01 shows the semantic compile checks for unknown columns and type mismatches. In a copy of
`waffle-shop`:

1. In `sql/sources/raw.yml`, add `contract: enforced` to `raw__customers` and `raw__orders`, so the
   columns and types that flow into the models are authoritative.
2. In `sql/models/marts/fact_orders.sql`, change `o.quantity,` to `o.qty,` and add
   `WHERE o.ordered_at > 5` as the last line.
3. Run `sqb compile`.

## Step 08: janitor

`tidy-shop` shows the state after the stale model was removed. To regenerate the output, restore it
first:

```bash
cd tidy-shop
cat > sql/models/weekly_revenue.sql <<'SQL'
MODEL (
  description "Weekly revenue totals",
  materialized table,
);

SELECT DATE_TRUNC('week', revenue_date) AS revenue_week, SUM(total_revenue_cents) AS total_revenue_cents
FROM __ref("daily_revenue")
GROUP BY 1
SQL
sqb build
rm sql/models/weekly_revenue.sql
sqb janitor
```

## Step 07: rename

`waffle-shop` shows the state after the rename. Give the model its old name, build it, then rename
it back:

```bash
cd waffle-shop
mv sql/models/marts/daily_order_rollup.sql sql/models/marts/daily_activity_rollup.sql
sed -i 's/daily_order_rollup/daily_activity_rollup/g' sql/models/marts/hourly_activity_with_daily_context.sql
sqb build
sqb rename daily_activity_rollup daily_order_rollup
sqb plan
```

## Hero: move and rename

`hero-shop` shows the state after the move. The model started as `sql/models/marts/revenue.sql`:

```bash
cd hero-shop
mkdir -p sql/models/marts
mv sql/models/finance/daily_revenue.sql sql/models/marts/revenue.sql
sqb build
sqb mv revenue sql/models/finance/daily_revenue.sql
sqb plan
```

`sqb rename` and `sqb mv` add `migrate_from` to the model header; the example projects leave it out,
so they can be set up again.
