MODEL (
  description """
    One row per live payment attempt with the gateway outcome extracted from its response
  """,
  columns (
    payment_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_id (type VARCHAR, audits [not_null]),
    payment_method (type VARCHAR),
    amount_minor (type INT, audits [not_null]),
    currency_code (type VARCHAR),
    authorised_at (type TIMESTAMP_TZ),
    captured_at (type TIMESTAMP_TZ),
    card_bin (type VARCHAR),
    gateway_status (type VARCHAR),
    gateway_error_code (type VARCHAR),
    risk_score (type NUMBER(5, 2)),
    three_ds_outcome (type VARCHAR),
    gateway_reference (type INT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("storefront__payments")
),

@sql.cdc_latest(src, src.key := (payment_id))

final AS (
  SELECT
    p.payment_id,
    p.order_id,
    LOWER(REGEXP_REPLACE(p.method, '[\\s-]+', '_'))::VARCHAR AS payment_method,
    p.amount_minor,
    UPPER(p.currency_code)::VARCHAR AS currency_code,
    p.authorised_at,
    p.captured_at,
    IFF(RLIKE(p.card_bin, '^[0-9]{6,8}$'), p.card_bin, NULL)::VARCHAR AS card_bin,
    LOWER(
      COALESCE(p.gateway_response:status.code, p.gateway_response:status)::VARCHAR
    )::VARCHAR AS gateway_status,
    REGEXP_SUBSTR(
      p.gateway_response:status.message::VARCHAR,
      '\\b([A-Z]{2}[0-9]{3})\\b',
      1,
      1,
      'e',
      1
    )::VARCHAR AS gateway_error_code,
    TRY_TO_DECIMAL(p.gateway_response:risk.score::VARCHAR, 5, 2)::NUMBER(5, 2) AS risk_score,
    UPPER(p.gateway_response:three_ds.outcome::VARCHAR)::VARCHAR AS three_ds_outcome,
    TRY_TO_NUMBER(p.gateway_response:reference::VARCHAR)::INT AS gateway_reference,
    p.gateway_response::OBJECT AS gateway_response,
    @sql.load_metadata(p)
  FROM cdc_latest AS p
)
SELECT *
FROM final
