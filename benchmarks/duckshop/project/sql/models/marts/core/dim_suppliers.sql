MODEL (
  description """
    One row per supplier with payment terms parsed from the terms object, lead-time and fill-rate
    performance, and the number of SKUs it currently supplies.
  """,
  contract enforced,
  model_schema audit_ts,
  columns (
    supplier_code (type VARCHAR, nullable false, audits [not_null, unique]),
    supplier_name (type VARCHAR),
    country_code (type VARCHAR),
    payment_terms_days (type INT),
    early_payment_discount_pct (type NUMBER(5, 2)),
    terms_currency_code (type VARCHAR),
    contact_domain (type VARCHAR),
    supplied_sku_count (type INT),
    lead_time_p50_days (type NUMBER(10, 2)),
    on_time_rate (type NUMBER(9, 4)),
    avg_fill_rate (type NUMBER(9, 4)),
    performance_band (type VARCHAR),
  ),
);

WITH suppliers AS (
  SELECT *
  FROM __ref("stg_suppliers")
),

lead_times AS (
  SELECT *
  FROM __ref("int_supplier_lead_times")
),

costs AS (
  SELECT *
  FROM __ref("int_supplier_cost_current")
),

sku_counts AS (
  SELECT
    c.supplier_code,
    COUNT(*) AS supplied_sku_count
  FROM costs AS c
  GROUP BY
    c.supplier_code
),

final AS (
  SELECT
    s.supplier_code,
    s.supplier_name,
    s.country_code,
    s.payment_terms_days,
    s.early_payment_discount_pct,
    UPPER(s.payment_terms:currency::VARCHAR)::VARCHAR AS terms_currency_code,
    LOWER(SPLIT_PART(s.primary_contact_email, '@', 2))::VARCHAR AS contact_domain,
    COALESCE(k.supplied_sku_count, 0)::INT AS supplied_sku_count,
    l.lead_time_p50_days,
    l.on_time_rate,
    l.avg_fill_rate,
    CASE
      WHEN l.on_time_rate IS NULL
      THEN 'unrated'
      WHEN l.on_time_rate >= 0.95
      AND l.avg_fill_rate >= 0.98
      THEN 'preferred'
      WHEN l.on_time_rate >= 0.8
      THEN 'approved'
      ELSE 'at_risk'
    END::VARCHAR AS performance_band,
    s.source_loaded_at
  FROM suppliers AS s
  LEFT JOIN lead_times AS l
    ON s.supplier_code = l.supplier_code
  LEFT JOIN sku_counts AS k
    ON s.supplier_code = k.supplier_code
)

SELECT *
FROM final
