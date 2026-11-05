MODEL (
  description """
    One row per reconciliation check with its key and narrow key in an object literal, and its
    FLOAT ratio passed through TRY_CAST to FLOAT
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    check_json (type VARCHAR),
    ratio_again (type FLOAT),
  ),
);

WITH check_objects AS (
  SELECT
    a.check_id,
    {'key': a.key_text, 'keys': {'narrow': a.narrow_key}} AS check_object,
    a.ratio
  FROM __ref("stg_edge_amount_checks") AS a
),

final AS (
  SELECT
    o.check_id,
    TO_JSON(o.check_object) AS check_json,
    TRY_CAST(o.ratio AS FLOAT) AS ratio_again
  FROM check_objects AS o
)

SELECT *
FROM final
