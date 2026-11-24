MODEL (
  description "One row per discount applied to an order line, flattened from the discounts array",
  columns (
    order_line_discount_key (type VARCHAR, nullable false, audits [not_null, unique]),
    order_line_id (type VARCHAR, audits [not_null]),
    order_id (type VARCHAR),
    discount_code (type VARCHAR),
    discount_kind (type VARCHAR),
    discount_amount_minor (type INT),
  ),
);

WITH order_lines AS (
  SELECT *
  FROM __ref("stg_order_lines")
),

flattened AS (
  SELECT
    l.order_line_id,
    l.order_id,
    d.index::INT AS discount_index,
    d.value AS discount
  FROM order_lines AS l, LATERAL FLATTEN(input => l.discounts) AS d
  WHERE IS_ARRAY(l.discounts)
),

final AS (
  SELECT
    (
      f.order_line_id || '#' || f.discount_index::VARCHAR
    )::VARCHAR AS order_line_discount_key,
    f.order_line_id,
    f.order_id,
    UPPER(@sql.json_text(f.discount:code))::VARCHAR AS discount_code,
    LOWER(COALESCE(@sql.json_text(f.discount:kind), 'manual'))::VARCHAR AS discount_kind,
    TRY_TO_NUMBER(f.discount:amount_minor::VARCHAR)::INT AS discount_amount_minor
  FROM flattened AS f
)

SELECT *
FROM final
