MODEL (
  description """
    One row per reconciliation check, keyed by a NUMBER(18,0), a NUMBER(38,0) and a text key, with
    amounts at scales 2 and 15, a FLOAT ratio and raw JSON number text
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    group_code (type VARCHAR),
    narrow_key (type NUMBER(18, 0)),
    wide_key (type NUMBER(38, 0)),
    key_text (type VARCHAR),
    amount (type NUMBER(18, 2)),
    fine_amount (type NUMBER(20, 15)),
    ratio (type FLOAT),
    json_text (type VARCHAR),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("edge__amount_checks")
),

final AS (
  SELECT
    s.check_id,
    s.group_code,
    s.narrow_key,
    s.wide_key,
    s.key_text,
    s.amount,
    s.fine_amount,
    s.ratio,
    s.json_text
  FROM src AS s
)

SELECT *
FROM final
