MODEL (
  description """
    One row per reconciliation check with its FLOAT ratio read back from an object as an INTEGER
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    ratio_integer (type INT),
  ),
);

WITH ratio_objects AS (
  SELECT a.check_id, OBJECT_CONSTRUCT('r', a.ratio) AS ratio_object
  FROM __ref("stg_edge_amount_checks") AS a
),

final AS (
  SELECT
    r.check_id,
    r.ratio_object:r::INTEGER AS ratio_integer
  FROM ratio_objects AS r
)

SELECT *
FROM final
