MODEL (
  description """
    One row per reconciliation check with a positive-ratio flag read back as a BOOLEAN from an
    object that also holds the FLOAT ratio
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    ratio_flag (type BOOLEAN),
  ),
);

WITH ratio_objects AS (
  SELECT a.check_id, OBJECT_CONSTRUCT('r', a.ratio, 'is_positive', a.ratio > 0) AS ratio_object
  FROM __ref("stg_edge_amount_checks") AS a
),

final AS (
  SELECT
    r.check_id,
    r.ratio_object:is_positive::BOOLEAN AS ratio_flag
  FROM ratio_objects AS r
)

SELECT *
FROM final
