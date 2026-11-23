MODEL (
  description "One row per purchase order line, flattened from the order's lines array",
  columns (
    po_line_key (type VARCHAR, nullable false, audits [not_null, unique]),
    po_id (type VARCHAR, audits [not_null]),
    sku (type VARCHAR, audits [not_null]),
    ordered_qty (type INT),
    unit_cost_minor (type INT),
    currency_code (type VARCHAR),
  ),
);

WITH purchase_orders AS (
  SELECT *
  FROM __ref("stg_purchase_orders")
),

lines AS (
  SELECT
    p.po_id,
    p.currency_code,
    l.index::INT AS line_index,
    l.value AS line
  FROM purchase_orders AS p, LATERAL FLATTEN(input => p.lines) AS l
),

final AS (
  SELECT
    (
      l.po_id || '-' || LPAD(l.line_index::VARCHAR, 3, '0')
    )::VARCHAR AS po_line_key,
    l.po_id,
    @sql.normalise_sku(l.line:sku::VARCHAR) AS sku,
    l.line:qty::INT AS ordered_qty,
    COALESCE(l.line:unit_cost_minor, l.line:unit_cost)::INT AS unit_cost_minor,
    COALESCE(UPPER(l.line:currency::VARCHAR), l.currency_code)::VARCHAR AS currency_code
  FROM lines AS l
)

SELECT *
FROM final
