MODEL (
  description """
    One row per reconciliation check with a branch on a path of its JSON text, whether two
    arrays holding a DATE differ, and the group's largest ratio held as VARIANT
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    ratio_branch (type NUMBER(38, 0)),
    dates_differ (type BOOLEAN),
    largest_ratio_text (type VARCHAR),
  ),
);

WITH largest AS (
  SELECT
    a.group_code,
    MAX(TO_VARIANT(a.ratio)) AS largest_ratio
  FROM __ref("stg_edge_amount_checks") AS a
  GROUP BY
    a.group_code
),

final AS (
  SELECT
    a.check_id,
    CASE PARSE_JSON(a.json_text):r WHEN 0.1 THEN 1 ELSE 2 END AS ratio_branch,
    (ARRAY_CONSTRUCT(a.check_id, TO_DATE('2026-01-01')) <> ARRAY_CONSTRUCT(a.check_id, TO_DATE('2026-01-02')))::BOOLEAN
      AS dates_differ,
    m.largest_ratio::VARCHAR AS largest_ratio_text
  FROM __ref("stg_edge_amount_checks") AS a
  INNER JOIN largest AS m
    ON a.group_code = m.group_code
)

SELECT *
FROM final
