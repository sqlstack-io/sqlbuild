MODEL (
  description """
    One row per check group counting the checks whose id and key text are both present, with
    COUNT over two arguments
  """,
  tags [edge_case],
  columns (
    group_code (type VARCHAR, nullable false, audits [not_null, unique]),
    complete_pair_count (type NUMBER(38, 0)),
  ),
);

WITH final AS (
  SELECT
    a.group_code,
    COUNT(a.check_id, a.key_text)::NUMBER(38, 0) AS complete_pair_count
  FROM __ref("stg_edge_amount_checks") AS a
  GROUP BY
    a.group_code
)

SELECT *
FROM final
