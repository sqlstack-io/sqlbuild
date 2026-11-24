MODEL (
  description """
    One row per gateway callback with a nanosecond timestamp built from its wall time in
    microseconds, and the wall time's epoch microseconds
  """,
  tags [edge_case],
  columns (
    callback_id (type VARCHAR, nullable false, audits [not_null, unique]),
    wall_time_ns (type TIMESTAMP_NTZ(9)),
    epoch_microsecond (type NUMBER(38, 0)),
  ),
);

WITH final AS (
  SELECT
    g.callback_id,
    TO_TIMESTAMP_NTZ(DATE_PART(EPOCH_SECOND, g.local_wall_time), 0)::TIMESTAMP_NTZ(9) AS wall_time_ns,
    DATE_PART(EPOCH_MICROSECOND, g.local_wall_time)::NUMBER(38, 0) AS epoch_microsecond
  FROM __ref("stg_edge_gateway_callbacks") AS g
)

SELECT *
FROM final
