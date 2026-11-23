MODEL (
  description """
    One row per order ledger line, with the quantity text read in every number spelling and the
    status kept as sent
  """,
  tags [edge_case],
  columns (
    entry_id (type VARCHAR, nullable false, audits [not_null, unique]),
    order_ref (type VARCHAR),
    channel (type VARCHAR),
    line_no (type NUMBER(38, 0)),
    sku (type VARCHAR),
    warehouse_code (type VARCHAR),
    quantity (type NUMBER(38, 0)),
    quantity_text (type VARCHAR),
    quantity_from_text (type NUMBER(18, 2)),
    quantity_text_decimal (type NUMBER(18, 0)),
    unit_price_minor (type NUMBER(38, 0)),
    amount (type NUMBER(38, 10)),
    currency_code (type VARCHAR),
    fx_rate (type FLOAT),
    status (type VARCHAR),
    reason_code (type VARCHAR),
    customer_tier (type VARCHAR),
    changed_at (type TIMESTAMP_NTZ),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("edge__ledger_entries")
),

final AS (
  SELECT
    s.entry_id,
    s.order_ref,
    s.channel,
    s.line_no,
    s.sku,
    s.warehouse_code,
    s.quantity,
    s.quantity_text,
    TRY_TO_NUMBER(s.quantity_text, 18, 2)::NUMBER(18, 2) AS quantity_from_text,
    TRY_TO_DECIMAL(s.quantity_text)::NUMBER(18, 0) AS quantity_text_decimal,
    s.unit_price_minor,
    s.amount,
    s.currency_code,
    s.fx_rate,
    s.status,
    s.reason_code,
    s.customer_tier,
    s.changed_at
  FROM src AS s
)

SELECT *
FROM final
