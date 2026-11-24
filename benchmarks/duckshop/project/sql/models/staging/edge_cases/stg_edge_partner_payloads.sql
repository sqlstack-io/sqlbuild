MODEL (
  description """
    One row per partner payload message, from its latest delivery, with payload fields read through
    paths, subscripts and typed casts, and the raw message text parsed leniently
  """,
  tags [edge_case],
  columns (
    message_id (type VARCHAR, nullable false, audits [not_null, unique]),
    partner_code (type VARCHAR),
    received_at (type TIMESTAMP_NTZ),
    payload (type VARIANT),
    payload_text (type VARCHAR),
    text_payload (type VARIANT),
    qty_kind (type VARCHAR),
    qty_is_integer (type BOOLEAN),
    qty_is_decimal (type BOOLEAN),
    qty_is_double (type BOOLEAN),
    qty_units (type NUMBER(18, 0)),
    qty_text_units (type NUMBER(18, 2)),
    amount (type NUMBER(18, 2)),
    amount_float (type FLOAT),
    amount_text (type VARCHAR),
    transaction_id_text (type VARCHAR),
    transaction_id (type NUMBER(38, 0)),
    is_gift (type BOOLEAN),
    placed_on (type DATE),
    first_item_sku (type VARCHAR),
    last_item_sku (type VARCHAR),
    item_count (type INT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("edge__partner_payloads")
),

latest AS (
  SELECT
    s.message_id,
    UPPER(TRIM(s.partner_code))::VARCHAR AS partner_code,
    s.received_at,
    s.payload,
    s.payload_text
  FROM src AS s
  QUALIFY
    ROW_NUMBER() OVER (PARTITION BY s.message_id ORDER BY s._loaded_at DESC, s.delivery_id DESC) = 1
),

final AS (
  SELECT
    l.message_id,
    l.partner_code,
    l.received_at,
    l.payload,
    l.payload_text,
    TRY_PARSE_JSON(l.payload_text) AS text_payload,
    TYPEOF(l.payload:qty)::VARCHAR AS qty_kind,
    IS_INTEGER(l.payload:qty)::BOOLEAN AS qty_is_integer,
    IS_DECIMAL(l.payload:qty)::BOOLEAN AS qty_is_decimal,
    IS_DOUBLE(l.payload:qty)::BOOLEAN AS qty_is_double,
    l.payload:qty::NUMBER(18, 0) AS qty_units,
    TRY_TO_NUMBER(l.payload:qty::VARCHAR, 18, 2)::NUMBER(18, 2) AS qty_text_units,
    l.payload:amount::NUMBER(18, 2) AS amount,
    l.payload:amount::FLOAT AS amount_float,
    l.payload:amount::VARCHAR AS amount_text,
    l.payload:txn_id::VARCHAR AS transaction_id_text,
    l.payload:txn_id::NUMBER(38, 0) AS transaction_id,
    l.payload:gift::BOOLEAN AS is_gift,
    l.payload:placed_on::DATE AS placed_on,
    l.payload:items[0]:sku::VARCHAR AS first_item_sku,
    l.payload:items[ARRAY_SIZE(l.payload:items) - 1]:sku::VARCHAR AS last_item_sku,
    ARRAY_SIZE(l.payload:items)::INT AS item_count
  FROM latest AS l
)

SELECT *
FROM final
