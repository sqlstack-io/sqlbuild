MODEL (
  description """
    One row per stock change of a SKU in a warehouse from every source (movements, purchase order
    receipts and transfers in and out), in UTC, with the running on-hand quantity.
  """,
  columns (
    ledger_entry_key (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR, audits [not_null]),
    warehouse_code (type VARCHAR, audits [not_null]),
    entry_source (type VARCHAR, audits [
      accepted_values (values ["movement", "receipt", "transfer_out", "transfer_in"]),
    ]),
    qty_delta (type INT),
    occurred_at_utc (type TIMESTAMP_NTZ),
    occurred_date_local (type DATE),
    running_on_hand (type INT),
    entry_sequence (type INT),
    is_negative_balance (type BOOLEAN),
  ),
);

WITH movements AS (
  SELECT *
  FROM __ref("stg_stock_movements")
),

receipts AS (
  SELECT *
  FROM __ref("stg_po_receipts")
),

transfers AS (
  SELECT *
  FROM __ref("stg_stock_transfers")
),

warehouses AS (
  SELECT *
  FROM __ref("stg_warehouses")
),

movement_entries AS (
  SELECT
    (
      'M:' || m.movement_id
    )::VARCHAR AS ledger_entry_key,
    m.sku::VARCHAR AS sku,
    m.warehouse_code::VARCHAR AS warehouse_code,
    'movement'::VARCHAR AS entry_source,
    m.qty_delta::INT AS qty_delta,
    CONVERT_TIMEZONE(w.iana_timezone, 'UTC', m.moved_at_local)::TIMESTAMP_NTZ AS occurred_at_utc,
    m.moved_at_local::DATE AS occurred_date_local
  FROM movements AS m
  INNER JOIN warehouses AS w
    ON m.warehouse_code = w.warehouse_code
  WHERE m.reference_type <> 'PO'
),

receipt_entries AS (
  SELECT
    (
      'R:' || r.receipt_id
    )::VARCHAR AS ledger_entry_key,
    r.sku::VARCHAR AS sku,
    r.warehouse_code::VARCHAR AS warehouse_code,
    'receipt'::VARCHAR AS entry_source,
    r.sellable_qty::INT AS qty_delta,
    CONVERT_TIMEZONE(w.iana_timezone, 'UTC', r.received_at_local)::TIMESTAMP_NTZ AS occurred_at_utc,
    r.received_at_local::DATE AS occurred_date_local
  FROM receipts AS r
  INNER JOIN warehouses AS w
    ON r.warehouse_code = w.warehouse_code
),

transfer_out_entries AS (
  SELECT
    (
      'TO:' || t.transfer_id
    )::VARCHAR AS ledger_entry_key,
    t.sku::VARCHAR AS sku,
    t.from_warehouse_code::VARCHAR AS warehouse_code,
    'transfer_out'::VARCHAR AS entry_source,
    (
      -t.qty
    )::INT AS qty_delta,
    CONVERT_TIMEZONE('UTC', t.shipped_at)::TIMESTAMP_NTZ AS occurred_at_utc,
    CONVERT_TIMEZONE(w.iana_timezone, t.shipped_at)::DATE AS occurred_date_local
  FROM transfers AS t
  INNER JOIN warehouses AS w
    ON t.from_warehouse_code = w.warehouse_code
  WHERE t.shipped_at IS NOT NULL
),

transfer_in_entries AS (
  SELECT
    (
      'TI:' || t.transfer_id
    )::VARCHAR AS ledger_entry_key,
    t.sku::VARCHAR AS sku,
    t.to_warehouse_code::VARCHAR AS warehouse_code,
    'transfer_in'::VARCHAR AS entry_source,
    t.qty::INT AS qty_delta,
    CONVERT_TIMEZONE('UTC', t.arrived_at)::TIMESTAMP_NTZ AS occurred_at_utc,
    CONVERT_TIMEZONE(w.iana_timezone, t.arrived_at)::DATE AS occurred_date_local
  FROM transfers AS t
  INNER JOIN warehouses AS w
    ON t.to_warehouse_code = w.warehouse_code
  WHERE t.arrived_at IS NOT NULL
),

all_entries AS (
  SELECT
    me.ledger_entry_key::VARCHAR AS ledger_entry_key,
    me.sku::VARCHAR AS sku,
    me.warehouse_code::VARCHAR AS warehouse_code,
    me.entry_source::VARCHAR AS entry_source,
    me.qty_delta::INT AS qty_delta,
    me.occurred_at_utc::TIMESTAMP_NTZ AS occurred_at_utc,
    me.occurred_date_local::DATE AS occurred_date_local
  FROM movement_entries AS me
  UNION ALL
  SELECT
    re.ledger_entry_key::VARCHAR AS ledger_entry_key,
    re.sku::VARCHAR AS sku,
    re.warehouse_code::VARCHAR AS warehouse_code,
    re.entry_source::VARCHAR AS entry_source,
    re.qty_delta::INT AS qty_delta,
    re.occurred_at_utc::TIMESTAMP_NTZ AS occurred_at_utc,
    re.occurred_date_local::DATE AS occurred_date_local
  FROM receipt_entries AS re
  UNION ALL
  SELECT
    toe.ledger_entry_key::VARCHAR AS ledger_entry_key,
    toe.sku::VARCHAR AS sku,
    toe.warehouse_code::VARCHAR AS warehouse_code,
    toe.entry_source::VARCHAR AS entry_source,
    toe.qty_delta::INT AS qty_delta,
    toe.occurred_at_utc::TIMESTAMP_NTZ AS occurred_at_utc,
    toe.occurred_date_local::DATE AS occurred_date_local
  FROM transfer_out_entries AS toe
  UNION ALL
  SELECT
    tie.ledger_entry_key::VARCHAR AS ledger_entry_key,
    tie.sku::VARCHAR AS sku,
    tie.warehouse_code::VARCHAR AS warehouse_code,
    tie.entry_source::VARCHAR AS entry_source,
    tie.qty_delta::INT AS qty_delta,
    tie.occurred_at_utc::TIMESTAMP_NTZ AS occurred_at_utc,
    tie.occurred_date_local::DATE AS occurred_date_local
  FROM transfer_in_entries AS tie
),

final AS (
  SELECT
    a.ledger_entry_key,
    a.sku,
    a.warehouse_code,
    a.entry_source,
    a.qty_delta,
    a.occurred_at_utc,
    a.occurred_date_local,
    SUM(a.qty_delta) OVER (
      PARTITION BY a.sku, a.warehouse_code
      ORDER BY a.occurred_at_utc, a.ledger_entry_key
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    )::INT AS running_on_hand,
    COUNT(*) OVER (
      PARTITION BY a.sku, a.warehouse_code
      ORDER BY a.occurred_at_utc, a.ledger_entry_key
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    )::INT AS entry_sequence,
    (
      SUM(a.qty_delta) OVER (
        PARTITION BY a.sku, a.warehouse_code
        ORDER BY a.occurred_at_utc, a.ledger_entry_key
        ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
      ) < 0
    ) AS is_negative_balance
  FROM all_entries AS a
)

SELECT *
FROM final
