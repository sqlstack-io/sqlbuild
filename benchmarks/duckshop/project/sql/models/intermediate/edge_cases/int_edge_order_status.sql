MODEL (
  description """
    One row per order with its status upper-cased, preferring a shipped ledger line, where the
    window ordering names the raw status column that the upper-cased select alias shadows
  """,
  tags [edge_case],
  columns (
    order_ref (type VARCHAR, nullable false, audits [not_null, unique]),
    status (type VARCHAR),
    entry_id (type VARCHAR),
  ),
);

WITH lines AS (
  SELECT
    e.entry_id,
    e.order_ref,
    e.status
  FROM __ref("stg_edge_ledger_entries") AS e
  WHERE e.order_ref IS NOT NULL
),

final AS (
  SELECT
    l.order_ref,
    UPPER(l.status)::VARCHAR AS status,
    l.entry_id
  FROM lines AS l
  QUALIFY
    ROW_NUMBER() OVER (PARTITION BY l.order_ref ORDER BY status = 'SHIPPED' DESC, l.entry_id) = 1
)

SELECT *
FROM final
