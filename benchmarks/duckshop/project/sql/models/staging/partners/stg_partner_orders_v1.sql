MODEL (
  description """
    One row per partner order received in payload version 1, where amounts are decimal strings
  """,
  columns (
    partner_order_key (type VARCHAR, nullable false, audits [not_null, unique]),
    partner_code (type VARCHAR, audits [not_null]),
    external_order_id (type VARCHAR, audits [not_null]),
    buyer_email (type VARCHAR),
    currency_code (type VARCHAR),
    order_total (type NUMBER(18, 3)),
    placed_at (type TIMESTAMP_TZ),
    lines (type ARRAY),
  ),
);

WITH messages AS (
  SELECT *
  FROM __ref("stg_partner_orders")
),

final AS (
  SELECT
    (
      m.partner_code || ':' || m.external_order_id
    )::VARCHAR AS partner_order_key,
    m.partner_code,
    m.external_order_id,
    @sql.normalise_email(m.payload:buyer.email::VARCHAR) AS buyer_email,
    UPPER(m.payload:total.currency::VARCHAR)::VARCHAR AS currency_code,
    TO_DECIMAL(m.payload:total.amount::VARCHAR, 18, 3)::NUMBER(18, 3) AS order_total,
    TO_TIMESTAMP_TZ(m.payload:created::VARCHAR) AS placed_at,
    COALESCE(m.payload:lines, ARRAY_CONSTRUCT())::ARRAY AS lines
  FROM messages AS m
  WHERE m.schema_version = 1
)

SELECT *
FROM final
