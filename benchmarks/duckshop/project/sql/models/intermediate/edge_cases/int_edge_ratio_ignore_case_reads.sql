MODEL (
  description """
    One row per reconciliation check with its FLOAT ratio read back from an object by
    GET_IGNORE_CASE
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    ratio_again (type FLOAT),
  ),
);

WITH ratio_objects AS (
  SELECT a.check_id, OBJECT_CONSTRUCT('Ratio', a.ratio) AS ratio_object
  FROM __ref("stg_edge_amount_checks") AS a
),

final AS (
  SELECT
    r.check_id,
    GET_IGNORE_CASE(r.ratio_object, 'ratio')::FLOAT AS ratio_again
  FROM ratio_objects AS r
)

SELECT *
FROM final
