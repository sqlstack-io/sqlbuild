MODEL (
  description """
    One row per reconciliation check whose ratio object differs from a fixed ratio object
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    check_json (type VARCHAR),
  ),
);

WITH differences AS (
  SELECT a.check_id, OBJECT_CONSTRUCT('r', a.ratio) AS ratio_object
  FROM __ref("stg_edge_amount_checks") AS a
  EXCEPT
  SELECT 'AC-01' AS check_id, OBJECT_CONSTRUCT('r', 0.1::FLOAT) AS ratio_object
),

final AS (
  SELECT
    d.check_id,
    TO_JSON(d.ratio_object) AS check_json
  FROM differences AS d
)

SELECT *
FROM final
