MODEL (
  description """
    One row per check group with the last FLOAT ratio of the group read by position from an
    ordered array
  """,
  tags [edge_case],
  columns (
    group_code (type VARCHAR, nullable false, audits [not_null, unique]),
    last_ratio (type FLOAT),
  ),
);

WITH ratio_arrays AS (
  SELECT
    a.group_code,
    ARRAY_AGG(a.ratio) WITHIN GROUP (ORDER BY a.check_id) AS ratios,
    COUNT(a.ratio) AS ratio_count
  FROM __ref("stg_edge_amount_checks") AS a
  GROUP BY
    a.group_code
),

final AS (
  SELECT
    r.group_code,
    r.ratios[r.ratio_count - 1]::FLOAT AS last_ratio
  FROM ratio_arrays AS r
)

SELECT *
FROM final
