TEST (name "rpt_edge_planner_fan_in__keeps_only_entries_in_every_branch");

WITH __ref__stg_edge_ledger_entries AS (
  SELECT v.entry_id
  FROM VALUES ('LE-01'), ('LE-02'), ('LE-03') AS v(entry_id)
),

__ref__int_edge_line_amounts AS (
  SELECT
    v.entry_id,
    v.unit_amount::DECIMAL(38, 10) AS unit_amount
  FROM VALUES ('LE-01', 2.50), ('LE-02', 7.25), ('LE-04', 9.50) AS v(entry_id, unit_amount)
),

__ref__int_edge_orders_all_channels AS (
  SELECT
    v.entry_id,
    v.channel
  FROM VALUES ('LE-01', 'web'), ('LE-03', 'app'), ('LE-04', 'partner') AS v(entry_id, channel)
),

__expected__rpt_edge_planner_fan_in AS (
  SELECT
    'LE-01' AS entry_id,
    'web' AS channel,
    2.50::DECIMAL(38, 10) AS unit_amount
)

SELECT 1
