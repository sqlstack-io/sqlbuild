MODEL (
  description """
    One row per reconciliation check with its FLOAT ratio written by TO_CHAR with a format and
    as a run of stars as long as ten times the ratio, and the exponential of its narrow key
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    ratio_text (type VARCHAR),
    ratio_stars (type VARCHAR),
    narrow_key_exp (type FLOAT),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    TO_CHAR(a.ratio, '990.99') AS ratio_text,
    REPEAT('*', a.ratio * 10)::VARCHAR AS ratio_stars,
    EXP(a.narrow_key)::FLOAT AS narrow_key_exp
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
