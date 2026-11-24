MODEL (
  description """
    One row per partner sales channel order message, deduplicated and kept as raw payload by schema
    version
  """,
  columns (
    message_id (type VARCHAR, nullable false, audits [not_null, unique]),
    partner_code (type VARCHAR, audits [not_null]),
    schema_version (type INT, audits [accepted_values (values [1, 2])]),
    received_at (type TIMESTAMP_NTZ, audits [not_null]),
    external_order_id (type VARCHAR, audits [not_null]),
    payload (type OBJECT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("partners__partner_orders")
),

messages AS (
  SELECT
    m.message_id,
    UPPER(m.partner_code) AS partner_code,
    m.schema_version::INT AS schema_version,
    m.received_at,
    COALESCE(
      @sql.json_text(m.payload:orderId),
      @sql.json_text(m.payload:order.id)
    )::VARCHAR AS external_order_id,
    m.payload::OBJECT AS payload
  FROM src AS m
  QUALIFY ROW_NUMBER() OVER (PARTITION BY m.message_id ORDER BY m._loaded_at DESC) = 1
),

final AS (
  SELECT
    m.message_id,
    m.partner_code::VARCHAR AS partner_code,
    m.schema_version,
    m.received_at,
    m.external_order_id,
    m.payload
  FROM messages AS m
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY m.partner_code, m.external_order_id
      ORDER BY m.received_at DESC, m.message_id DESC
    ) = 1
)

SELECT *
FROM final
