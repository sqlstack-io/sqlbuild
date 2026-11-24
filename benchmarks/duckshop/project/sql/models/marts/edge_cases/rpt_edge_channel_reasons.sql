MODEL (
  description """
    One row per sales channel, including channels with no ledger lines and channels whose lines
    carry no reason code, with every aggregate over those empty and all-missing groups
  """,
  tags [edge_case],
  columns (
    channel (type VARCHAR, nullable false, audits [not_null, unique]),
    lines (type INT),
    reasons (type INT),
    refunded_lines (type INT),
    any_refund (type BOOLEAN),
    total_quantity (type NUMBER(38, 0)),
    max_reason (type VARCHAR),
    reason_list (type VARCHAR),
    reason_array (type ARRAY),
    reasons_by_entry (type OBJECT),
  ),
);

WITH channels AS (
  SELECT v.channel
  FROM VALUES ('web'), ('app'), ('partner'), ('phone') AS v(channel)
),

lines AS (
  SELECT
    e.entry_id,
    e.channel,
    e.quantity,
    e.reason_code
  FROM __ref("stg_edge_ledger_entries") AS e
),

final AS (
  SELECT
    c.channel::VARCHAR AS channel,
    COUNT(l.entry_id)::INT AS lines,
    COUNT(l.reason_code)::INT AS reasons,
    COUNT_IF(l.reason_code = 'REFUND')::INT AS refunded_lines,
    BOOLOR_AGG(l.reason_code = 'REFUND')::BOOLEAN AS any_refund,
    SUM(l.quantity)::NUMBER(38, 0) AS total_quantity,
    MAX(l.reason_code)::VARCHAR AS max_reason,
    LISTAGG(l.reason_code, ',') WITHIN GROUP (ORDER BY l.entry_id)::VARCHAR AS reason_list,
    ARRAY_AGG(l.reason_code) WITHIN GROUP (ORDER BY l.entry_id)::ARRAY AS reason_array,
    OBJECT_AGG(l.entry_id, l.reason_code::VARIANT)::OBJECT AS reasons_by_entry
  FROM channels AS c
  LEFT JOIN lines AS l
    ON c.channel = l.channel
  GROUP BY
    c.channel
)

SELECT *
FROM final
