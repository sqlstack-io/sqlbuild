MODEL (
  description """
    One row per bundle and currency with the summed list price of its required components, the
    bundle price under its pricing mode, and the saving offered.
  """,
  columns (
    bundle_price_key (type VARCHAR, nullable false, audits [not_null, unique]),
    bundle_sku (type VARCHAR, audits [not_null]),
    currency_code (type VARCHAR, audits [not_null]),
    component_count (type INT),
    components_list_minor (type INT),
    bundle_price_minor (type INT),
    saving_minor (type INT),
    saving_pct (type NUMBER(9, 4)),
    has_unpriced_component (type BOOLEAN),
  ),
);

WITH bundles AS (
  SELECT *
  FROM __ref("stg_bundles")
),

components AS (
  SELECT *
  FROM __ref("stg_bundle_components")
),

price_history AS (
  SELECT *
  FROM __ref("int_price_history")
),

current_prices AS (
  SELECT
    ph.sku,
    ph.currency_code,
    ph.list_amount_minor
  FROM price_history AS ph
  WHERE ph.effective_valid_to IS NULL
),

required_components AS (
  SELECT
    c.bundle_sku,
    c.component_sku,
    c.quantity
  FROM components AS c
  WHERE NOT c.is_optional
),

priced_components AS (
  SELECT
    rc.bundle_sku,
    cp.currency_code,
    rc.quantity * cp.list_amount_minor AS extended_minor,
    cp.list_amount_minor AS unit_list_minor
  FROM required_components AS rc
  INNER JOIN current_prices AS cp
    ON rc.component_sku = cp.sku
),

component_counts AS (
  SELECT
    rc.bundle_sku,
    COUNT(*) AS component_count
  FROM required_components AS rc
  GROUP BY
    rc.bundle_sku
),

totals AS (
  SELECT
    pc.bundle_sku,
    pc.currency_code,
    COUNT(*) AS priced_count,
    SUM(pc.extended_minor) AS components_list_minor,
    MIN(pc.unit_list_minor) AS cheapest_unit_minor
  FROM priced_components AS pc
  GROUP BY
    pc.bundle_sku,
    pc.currency_code
),

final AS (
  SELECT
    (
      t.bundle_sku || ':' || t.currency_code
    )::VARCHAR AS bundle_price_key,
    t.bundle_sku,
    t.currency_code,
    cc.component_count::INT AS component_count,
    t.components_list_minor::INT AS components_list_minor,
    CASE b.pricing_mode
      WHEN 'fixed'
      THEN IFF(b.pricing_currency_code = t.currency_code, b.fixed_amount_minor, NULL)
      WHEN 'percent_off'
      THEN ROUND(t.components_list_minor
      * (
        1 - b.percent_off / 100
      ))
      WHEN 'cheapest_free'
      THEN t.components_list_minor - t.cheapest_unit_minor
    END::INT AS bundle_price_minor,
    (
      t.components_list_minor
      - CASE b.pricing_mode
        WHEN 'fixed'
        THEN IFF(b.pricing_currency_code = t.currency_code, b.fixed_amount_minor, NULL)
        WHEN 'percent_off'
        THEN ROUND(t.components_list_minor
        * (
          1 - b.percent_off / 100
        ))
        WHEN 'cheapest_free'
        THEN t.components_list_minor - t.cheapest_unit_minor
      END
    )::INT AS saving_minor,
    ROUND(
      100
      * (
        1
        - CASE b.pricing_mode
          WHEN 'fixed'
          THEN IFF(b.pricing_currency_code = t.currency_code, b.fixed_amount_minor, NULL)
          WHEN 'percent_off'
          THEN ROUND(t.components_list_minor
          * (
            1 - b.percent_off / 100
          ))
          WHEN 'cheapest_free'
          THEN t.components_list_minor - t.cheapest_unit_minor
        END
        / NULLIF(t.components_list_minor, 0)
      ),
      4
    )::NUMBER(9, 4) AS saving_pct,
    (
      t.priced_count < cc.component_count
    ) AS has_unpriced_component
  FROM totals AS t
  INNER JOIN bundles AS b
    ON t.bundle_sku = b.bundle_sku
  INNER JOIN component_counts AS cc
    ON t.bundle_sku = cc.bundle_sku
)

SELECT *
FROM final
