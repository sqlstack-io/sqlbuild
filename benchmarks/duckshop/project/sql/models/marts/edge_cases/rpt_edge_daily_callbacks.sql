MODEL (
  description """
    One row per local callback date, counting callbacks and the distinct instants among them, where
    the same instant can arrive at different UTC offsets
  """,
  tags [edge_case],
  columns (
    event_local_date (type DATE, nullable false, audits [not_null, unique]),
    callbacks (type INT),
    distinct_instants (type INT),
    distinct_offsets (type INT),
    first_event_utc (type TIMESTAMP_NTZ),
  ),
);

WITH callback_times AS (
  SELECT
    t.callback_id,
    t.event_local_date,
    t.event_at_utc
  FROM __ref("int_edge_callback_times") AS t
),

callbacks AS (
  SELECT
    s.callback_id,
    s.event_at
  FROM __ref("stg_edge_gateway_callbacks") AS s
),

final AS (
  SELECT
    t.event_local_date,
    COUNT(*)::INT AS callbacks,
    COUNT(DISTINCT c.event_at)::INT AS distinct_instants,
    COUNT(DISTINCT TO_VARCHAR(c.event_at, 'TZH:TZM'))::INT AS distinct_offsets,
    MIN(t.event_at_utc) AS first_event_utc
  FROM callback_times AS t
  INNER JOIN callbacks AS c
    ON t.callback_id = c.callback_id
  WHERE t.event_local_date IS NOT NULL
  GROUP BY
    t.event_local_date
)

SELECT *
FROM final
