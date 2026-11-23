MODEL (
  description """
    One row per check group with the number of distinct (key text, ratio) arrays in it
  """,
  tags [edge_case],
  columns (
    group_code (type VARCHAR, nullable false, audits [not_null, unique]),
    distinct_tuples (type NUMBER(38, 0)),
  ),
);

WITH final AS (
  SELECT
    a.group_code,
    COUNT(DISTINCT ARRAY_CONSTRUCT(a.key_text, a.ratio::DOUBLE))::NUMBER(38, 0) AS distinct_tuples
  FROM __ref("stg_edge_amount_checks") AS a
  GROUP BY
    a.group_code
)

SELECT *
FROM final
