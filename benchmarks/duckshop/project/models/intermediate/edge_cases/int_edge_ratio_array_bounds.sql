MODEL (
  description """
    One row per check group with bounds read from the array of its FLOAT ratios
  """,
  tags [edge_case],
  columns (
    group_code (type VARCHAR, nullable false, audits [not_null, unique]),
    lower_bound (type FLOAT),
  ),
);

WITH ratio_arrays AS (
  SELECT
    a.group_code,
    ARRAY_AGG(a.ratio) WITHIN GROUP (ORDER BY a.check_id) AS ratios
  FROM __ref("stg_edge_amount_checks") AS a
  GROUP BY
    a.group_code
),

final AS (
  SELECT
    r.group_code,
    (-ARRAY_MAX(r.ratios) - 10.0)::FLOAT AS lower_bound
  FROM ratio_arrays AS r
)

SELECT *
FROM final
