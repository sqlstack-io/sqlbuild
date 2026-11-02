MODEL (
  description "One row per live promotion with eligibility rules and discount tiers extracted",
  columns (
    promo_code (type VARCHAR, nullable false, audits [not_null, unique]),
    promo_name (type VARCHAR),
    starts_at (type TIMESTAMP_TZ),
    ends_at (type TIMESTAMP_TZ),
    is_stackable (type BOOLEAN),
    min_order_minor (type INT),
    eligible_regions (type ARRAY),
    tier_count (type INT),
    max_discount_pct (type NUMBER(5, 2)),
    rules (type OBJECT),
    source_seq (type NUMBER(38, 0), nullable false),
    source_loaded_at (type TIMESTAMP_NTZ, nullable false),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__promotions")
),

@sql.cdc_latest(src, src.key := (promo_code))

final AS (
  SELECT
    UPPER(TRIM(p.promo_code))::VARCHAR AS promo_code,
    p.promo_name,
    p.starts_at,
    p.ends_at,
    COALESCE(p.is_stackable, FALSE) AS is_stackable,
    COALESCE(p.rules:eligibility.min_order_minor::INT, 0)::INT AS min_order_minor,
    COALESCE(p.rules:eligibility.regions, ARRAY_CONSTRUCT())::ARRAY AS eligible_regions,
    ARRAY_SIZE(COALESCE(p.rules:tiers, ARRAY_CONSTRUCT()))::INT AS tier_count,
    ARRAY_MAX(
      TRANSFORM(
        p.rules:tiers::ARRAY,
        t VARIANT -> t:pct
      )
    )::NUMBER(5, 2) AS max_discount_pct,
    p.rules::OBJECT AS rules,
    @sql.load_metadata(p)
  FROM cdc_latest AS p
)
SELECT *
FROM final
