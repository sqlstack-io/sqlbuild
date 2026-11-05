MODEL (
  description """
    One row per reconciliation check with a fixed number cast to text as a label
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    cluster_label (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    CAST(501 AS VARCHAR) AS cluster_label
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
