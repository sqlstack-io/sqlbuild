MODEL (
  description """
    One row per reconciliation check whose key span is known and whose FLOAT ratio, read back by
    path from an object, is missing
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    key_span (type NUMBER(38, 0)),
  ),
);

WITH ratio_objects AS (
  SELECT
    a.check_id,
    a.wide_key - a.narrow_key AS key_span,
    OBJECT_CONSTRUCT('r', a.ratio) AS ratio_object
  FROM __ref("stg_edge_amount_checks") AS a
),

final AS (
  SELECT
    r.check_id,
    r.key_span::NUMBER(38, 0) AS key_span
  FROM ratio_objects AS r
  WHERE r.ratio_object:r IS NULL
)

SELECT *
FROM final
