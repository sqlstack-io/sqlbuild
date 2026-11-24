MODEL (
  description """
    One row per reconciliation check with a key added to the first object of an array holding
    its FLOAT ratio
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    check_json (type VARCHAR),
  ),
);

WITH ratio_arrays AS (
  SELECT a.check_id, ARRAY_CONSTRUCT(OBJECT_CONSTRUCT('r', a.ratio)) AS ratio_objects
  FROM __ref("stg_edge_amount_checks") AS a
),

final AS (
  SELECT
    r.check_id,
    TO_JSON(OBJECT_INSERT(r.ratio_objects[0]::OBJECT, 'check', r.check_id)) AS check_json
  FROM ratio_arrays AS r
)

SELECT *
FROM final
