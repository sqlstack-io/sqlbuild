MODEL (
  description """
    One row per phone call with its IVR menu choices flattened, the final menu node, and whether the
    caller abandoned in the IVR
  """,
  columns (
    call_id (type VARCHAR, nullable false, audits [not_null, unique]),
    ticket_id (type VARCHAR),
    agent_id (type VARCHAR),
    first_menu_choice (type VARCHAR),
    final_menu_node (type VARCHAR),
    menu_steps (type INT),
    repeated_menu_visits (type INT),
    abandoned_in_ivr (type BOOLEAN),
    caller_region (type VARCHAR),
    duration_bucket (type VARCHAR),
  ),
);

WITH calls AS (
  SELECT *
  FROM __ref("stg_call_records")
),

call_paths AS (
  SELECT *
  FROM __source("support__call_records")
),

latest_paths AS (
  SELECT
    p.call_id,
    p.ivr_path
  FROM call_paths AS p
  QUALIFY ROW_NUMBER() OVER (PARTITION BY p.call_id ORDER BY p._seq DESC) = 1
),

steps AS (
  SELECT
    l.call_id,
    s.index AS step_index,
    LOWER(s.value::VARCHAR) AS menu_node
  FROM latest_paths AS l, LATERAL FLATTEN(input => l.ivr_path) AS s
),

step_summary AS (
  SELECT
    s.call_id,
    MIN_BY(s.menu_node, s.step_index) AS first_menu_choice,
    MAX_BY(s.menu_node, s.step_index) AS final_menu_node,
    COUNT(*) AS menu_steps,
    COUNT(*) - COUNT(DISTINCT s.menu_node) AS repeated_menu_visits
  FROM steps AS s
  GROUP BY
    s.call_id
),

final AS (
  SELECT
    c.call_id,
    c.ticket_id,
    c.agent_id,
    s.first_menu_choice::VARCHAR AS first_menu_choice,
    s.final_menu_node::VARCHAR AS final_menu_node,
    COALESCE(s.menu_steps, 0)::INT AS menu_steps,
    COALESCE(s.repeated_menu_visits, 0)::INT AS repeated_menu_visits,
    (
      c.agent_id IS NULL
      AND COALESCE(s.final_menu_node, '') NOT IN ('self_service_complete', 'callback_booked')
    ) AS abandoned_in_ivr,
    CASE c.caller_country_prefix
      WHEN '1'
      THEN 'US'
      WHEN '81'
      THEN 'APAC'
      WHEN '61'
      THEN 'APAC'
      WHEN '965'
      THEN 'MENA'
      ELSE 'EU'
    END::VARCHAR AS caller_region,
    CASE
      WHEN c.duration_seconds < 60
      THEN 'under_1m'
      WHEN c.duration_seconds < 300
      THEN '1_to_5m'
      WHEN c.duration_seconds < 900
      THEN '5_to_15m'
      ELSE 'over_15m'
    END::VARCHAR AS duration_bucket
  FROM calls AS c
  LEFT JOIN step_summary AS s
    ON c.call_id = s.call_id
)

SELECT *
FROM final
