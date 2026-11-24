MODEL (
  description """
    One row per ledger line across the web, app and partner channels, unioned by column name where
    each channel lists its columns in its own order, the partner channel has no rate and an untyped
    missing refund reason
  """,
  tags [edge_case],
  columns (
    entry_id (type VARCHAR, nullable false, audits [not_null, unique]),
    channel (type VARCHAR),
    order_ref (type VARCHAR),
    quantity (type NUMBER(38, 0)),
    amount (type NUMBER(38, 10)),
    refund_reason (type VARCHAR),
    rate (type NUMBER(38, 10)),
  ),
);

WITH lines AS (
  SELECT
    e.entry_id,
    e.order_ref,
    e.channel,
    e.quantity,
    e.amount,
    e.reason_code,
    e.fx_rate
  FROM __ref("stg_edge_ledger_entries") AS e
),

web AS (
  SELECT
    l.entry_id,
    l.channel,
    l.order_ref,
    l.quantity,
    l.amount,
    l.reason_code AS refund_reason,
    1::NUMBER(10, 5) AS rate
  FROM lines AS l
  WHERE l.channel = 'web'
),

app AS (
  SELECT
    l.amount,
    l.quantity,
    l.reason_code AS refund_reason,
    l.order_ref,
    l.channel,
    l.entry_id,
    l.fx_rate::NUMBER(10, 5) AS rate
  FROM lines AS l
  WHERE l.channel = 'app'
),

partner AS (
  SELECT
    l.entry_id,
    l.order_ref,
    l.channel,
    l.quantity,
    l.amount,
    NULL AS refund_reason,
    l.quantity::NUMBER(38, 0) AS rate
  FROM lines AS l
  WHERE l.channel = 'partner'
),

unioned AS (
  SELECT *
  FROM web
  UNION ALL BY NAME
  SELECT *
  FROM app
  UNION ALL BY NAME
  SELECT * EXCLUDE (rate)
  FROM partner
),

final AS (
  SELECT
    u.entry_id,
    u.channel,
    u.order_ref,
    u.quantity,
    u.amount,
    u.refund_reason::VARCHAR AS refund_reason,
    u.rate::NUMBER(38, 10) AS rate
  FROM unioned AS u
)

SELECT *
FROM final
