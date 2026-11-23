MODEL (
  description """
    One row per SKU with its cheapest current supplier cost converted to USD (ASOF the cost's
    effective date), the supplier, and how many suppliers quote it.
  """,
  columns (
    sku (type VARCHAR, nullable false, audits [not_null, unique]),
    supplier_code (type VARCHAR),
    supplier_name (type VARCHAR),
    currency_code (type VARCHAR),
    unit_cost_minor (type INT),
    unit_cost_usd (type NUMBER(18, 4)),
    effective_from (type DATE),
    supplier_count (type INT),
    payment_terms_days (type INT),
    cost_spread_pct (type NUMBER(9, 2)),
  ),
);

WITH supplier_costs AS (
  SELECT *
  FROM __ref("stg_supplier_costs")
),

suppliers AS (
  SELECT *
  FROM __ref("stg_suppliers")
),

fx_rates AS (
  SELECT *
  FROM __ref("stg_fx_rates")
),

currencies AS (
  SELECT *
  FROM __seed("currencies")
),

current_costs AS (
  SELECT
    c.cost_id,
    c.supplier_code,
    c.sku,
    c.currency_code,
    COALESCE(c.best_break_cost_minor, c.unit_cost_minor) AS unit_cost_minor,
    c.effective_from
  FROM supplier_costs AS c
  WHERE
    c.effective_to IS NULL
    OR c.effective_to >= '@@valuation_date'::DATE
),

with_exponent AS (
  SELECT
    cc.cost_id,
    cc.supplier_code,
    cc.sku,
    cc.currency_code,
    cc.unit_cost_minor,
    cc.effective_from,
    cur.minor_unit_exponent
  FROM current_costs AS cc
  INNER JOIN currencies AS cur
    ON cc.currency_code = cur.currency_code
),

in_usd AS (
  SELECT
    w.cost_id,
    w.supplier_code,
    w.sku,
    w.currency_code,
    w.unit_cost_minor,
    w.effective_from,
    ROUND(
      @sql.money_from_minor(w.unit_cost_minor, w.minor_unit_exponent, places := 6) * fx.rate_to_usd,
      4
    ) AS unit_cost_usd
  FROM with_exponent AS w
  ASOF JOIN fx_rates AS fx MATCH_CONDITION (w.effective_from >= fx.rate_date)
    ON w.currency_code = fx.currency_code
),

per_sku AS (
  SELECT
    u.sku,
    MIN_BY(u.cost_id, u.unit_cost_usd) AS cheapest_cost_id,
    COUNT(DISTINCT u.supplier_code) AS supplier_count,
    MAX(u.unit_cost_usd) AS max_cost_usd
  FROM in_usd AS u
  GROUP BY
    u.sku
),

ranked AS (
  SELECT
    p.sku,
    u.supplier_code,
    u.currency_code,
    u.unit_cost_minor,
    u.unit_cost_usd,
    u.effective_from,
    p.supplier_count,
    p.max_cost_usd
  FROM per_sku AS p
  INNER JOIN in_usd AS u
    ON p.cheapest_cost_id = u.cost_id
),

final AS (
  SELECT
    r.sku,
    r.supplier_code,
    s.supplier_name,
    r.currency_code,
    r.unit_cost_minor::INT AS unit_cost_minor,
    r.unit_cost_usd::NUMBER(18, 4) AS unit_cost_usd,
    r.effective_from,
    r.supplier_count::INT AS supplier_count,
    s.payment_terms_days,
    ROUND(100
    * (
      r.max_cost_usd - r.unit_cost_usd
    )
    / NULLIF(r.unit_cost_usd, 0), 2)::NUMBER(9, 2) AS cost_spread_pct
  FROM ranked AS r
  LEFT JOIN suppliers AS s
    ON r.supplier_code = s.supplier_code
)

SELECT *
FROM final
