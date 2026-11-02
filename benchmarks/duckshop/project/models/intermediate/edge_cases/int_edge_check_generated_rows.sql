MODEL (
  description """
    One row counting the rows generated, as many as there are reconciliation checks, with
    whether every SEQ4 number is non-negative and every draw from UNIFORM over a range of one
    value is one
  """,
  tags [edge_case],
  columns (
    summary_id (type VARCHAR, nullable false, audits [not_null, unique]),
    generated_count (type NUMBER(38, 0)),
    numbers_are_valid (type BOOLEAN),
    draws_are_one (type BOOLEAN),
  ),
);

WITH generated AS (
  SELECT
    SEQ4() AS row_seq,
    UNIFORM(1, 1, RANDOM()) AS draw
  FROM TABLE(GENERATOR(ROWCOUNT => (SELECT COUNT(*) FROM __ref("stg_edge_amount_checks"))))
),

final AS (
  SELECT
    'all' AS summary_id,
    COUNT(*)::NUMBER(38, 0) AS generated_count,
    BOOLAND_AGG(r.row_seq >= 0)::BOOLEAN AS numbers_are_valid,
    BOOLAND_AGG(r.draw = 1)::BOOLEAN AS draws_are_one
  FROM generated AS r
)

SELECT *
FROM final
