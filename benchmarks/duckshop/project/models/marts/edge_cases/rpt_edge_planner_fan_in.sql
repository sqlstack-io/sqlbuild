MODEL (
  description "Ledger report whose branches share an upstream model during migration planning",
  tags [edge_case],
  columns (
    entry_id (type VARCHAR, nullable false, audits [not_null, unique]),
    channel (type VARCHAR),
    unit_amount (type NUMBER(38, 10)),
  ),
);

WITH ledger AS (
  SELECT entry_id
  FROM __ref("stg_edge_ledger_entries")
),

amounts AS (
  SELECT
    entry_id,
    unit_amount
  FROM __ref("int_edge_line_amounts")
),

channels AS (
  SELECT
    entry_id,
    channel
  FROM __ref("int_edge_orders_all_channels")
),

final AS (
  SELECT
    l.entry_id,
    c.channel,
    a.unit_amount
  FROM ledger AS l
  INNER JOIN amounts AS a
    ON l.entry_id = a.entry_id
  INNER JOIN channels AS c
    ON l.entry_id = c.entry_id
)

SELECT
  entry_id,
  channel,
  unit_amount
FROM final
