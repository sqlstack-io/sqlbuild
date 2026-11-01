MODEL (
  description """
    One row per check with NULL arithmetic and literal typing: NULL written first, FLOAT / NULL,
    NUMBER(38,2) * NULL, a NULL column from a CTE, and the types Snowflake gives them, decimal and
    exponent literals, COUNT, ROW_NUMBER, % and NULLIF, a column NULL in every branch of a UNION,
    and a UNION with a decimal literal branch. Also CASE without ELSE and DECODE mixing text with
    numbers, where no branch reads the text that is not a number
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    null_plus_amount (type NUMBER(38, 2)),
    null_times_amount (type NUMBER(38, 2)),
    null_over_amount (type NUMBER(38, 6)),
    ratio_over_null (type FLOAT),
    wide_amount_times_null (type NUMBER(38, 2)),
    missing_times_amount (type NUMBER(38, 2)),
    null_times_amount_or_zero (type VARCHAR),
    null_over_amount_type (type VARCHAR),
    ratio_over_null_type (type VARCHAR),
    wide_amount_times_null_type (type VARCHAR),
    missing_times_amount_type (type VARCHAR),
    amount_or_decimal (type VARCHAR),
    amount_or_decimal_type (type VARCHAR),
    exponent_type (type VARCHAR),
    small_exponent_product_type (type VARCHAR),
    remainder_type (type VARCHAR),
    key_remainder_type (type VARCHAR),
    nullif_type (type VARCHAR),
    ilike_null_type (type VARCHAR),
    row_number_type (type VARCHAR),
    count_type (type VARCHAR),
    union_decimal_type (type VARCHAR),
    all_null_union_type (type VARCHAR),
    case_without_else (type VARCHAR),
    decoded (type VARCHAR),
  ),
);

WITH checks AS (
  SELECT
    c.check_id,
    c.narrow_key,
    c.key_text,
    c.amount,
    c.ratio
  FROM __ref("stg_edge_amount_checks") AS c
),

missing AS (
  SELECT NULL AS missing_amount
),

counted AS (
  SELECT SYSTEM$TYPEOF(COUNT(c.check_id)) AS count_type
  FROM checks AS c
),

with_decimal AS (
  SELECT c.amount AS mixed_amount
  FROM checks AS c
  UNION ALL
  SELECT 1.5 AS mixed_amount
),

union_typed AS (
  SELECT MAX(SYSTEM$TYPEOF(w.mixed_amount)) AS union_decimal_type
  FROM with_decimal AS w
),

all_null AS (
  SELECT NULL AS note
  UNION ALL
  SELECT NULL AS note
),

all_null_typed AS (
  SELECT MAX(SYSTEM$TYPEOF(a.note)) AS all_null_union_type
  FROM all_null AS a
),

computed AS (
  SELECT
    c.check_id,
    NULL + c.amount AS null_plus_amount,
    NULL * c.amount AS null_times_amount,
    NULL / c.amount AS null_over_amount,
    c.ratio / NULL AS ratio_over_null,
    c.amount::NUMBER(38, 2) * NULL AS wide_amount_times_null,
    m.missing_amount * c.amount AS missing_times_amount,
    COALESCE(NULL * c.amount, 0)::VARCHAR AS null_times_amount_or_zero,
    SYSTEM$TYPEOF(NULL / c.amount) AS null_over_amount_type,
    SYSTEM$TYPEOF(c.ratio / NULL) AS ratio_over_null_type,
    SYSTEM$TYPEOF(c.amount::NUMBER(38, 2) * NULL) AS wide_amount_times_null_type,
    SYSTEM$TYPEOF(m.missing_amount * c.amount) AS missing_times_amount_type,
    COALESCE(c.amount, 1.5)::VARCHAR AS amount_or_decimal,
    SYSTEM$TYPEOF(COALESCE(c.amount, 1.5)) AS amount_or_decimal_type,
    SYSTEM$TYPEOF(1e5) AS exponent_type,
    SYSTEM$TYPEOF(c.amount * 1e-12) AS small_exponent_product_type,
    SYSTEM$TYPEOF(7 % 3) AS remainder_type,
    SYSTEM$TYPEOF(c.narrow_key % 3) AS key_remainder_type,
    SYSTEM$TYPEOF(NULLIF(c.amount, 1)) AS nullif_type,
    SYSTEM$TYPEOF(c.key_text ILIKE NULL) AS ilike_null_type,
    SYSTEM$TYPEOF(ROW_NUMBER() OVER (ORDER BY c.check_id)) AS row_number_type,
    CASE
      WHEN c.narrow_key = 7
      THEN c.narrow_key
      WHEN c.narrow_key = 12
      THEN c.key_text
    END AS case_without_else,
    DECODE(c.narrow_key, 7, c.amount, 12, c.key_text) AS decoded
  FROM checks AS c
  CROSS JOIN missing AS m
),

final AS (
  SELECT
    c.check_id,
    c.null_plus_amount::NUMBER(38, 2) AS null_plus_amount,
    c.null_times_amount::NUMBER(38, 2) AS null_times_amount,
    c.null_over_amount::NUMBER(38, 6) AS null_over_amount,
    c.ratio_over_null::FLOAT AS ratio_over_null,
    c.wide_amount_times_null::NUMBER(38, 2) AS wide_amount_times_null,
    c.missing_times_amount::NUMBER(38, 2) AS missing_times_amount,
    c.null_times_amount_or_zero,
    REGEXP_REPLACE(c.null_over_amount_type, '\\[.*\\]$', '')::VARCHAR AS null_over_amount_type,
    REGEXP_REPLACE(c.ratio_over_null_type, '\\[.*\\]$', '')::VARCHAR AS ratio_over_null_type,
    REGEXP_REPLACE(
      c.wide_amount_times_null_type,
      '\\[.*\\]$',
      ''
    )::VARCHAR AS wide_amount_times_null_type,
    REGEXP_REPLACE(
      c.missing_times_amount_type,
      '\\[.*\\]$',
      ''
    )::VARCHAR AS missing_times_amount_type,
    c.amount_or_decimal,
    REGEXP_REPLACE(c.amount_or_decimal_type, '\\[.*\\]$', '')::VARCHAR AS amount_or_decimal_type,
    REGEXP_REPLACE(c.exponent_type, '\\[.*\\]$', '')::VARCHAR AS exponent_type,
    REGEXP_REPLACE(
      c.small_exponent_product_type,
      '\\[.*\\]$',
      ''
    )::VARCHAR AS small_exponent_product_type,
    REGEXP_REPLACE(c.remainder_type, '\\[.*\\]$', '')::VARCHAR AS remainder_type,
    REGEXP_REPLACE(c.key_remainder_type, '\\[.*\\]$', '')::VARCHAR AS key_remainder_type,
    REGEXP_REPLACE(c.nullif_type, '\\[.*\\]$', '')::VARCHAR AS nullif_type,
    REGEXP_REPLACE(c.ilike_null_type, '\\[.*\\]$', '')::VARCHAR AS ilike_null_type,
    REGEXP_REPLACE(c.row_number_type, '\\[.*\\]$', '')::VARCHAR AS row_number_type,
    REGEXP_REPLACE(k.count_type, '\\[.*\\]$', '')::VARCHAR AS count_type,
    REGEXP_REPLACE(u.union_decimal_type, '\\[.*\\]$', '')::VARCHAR AS union_decimal_type,
    REGEXP_REPLACE(n.all_null_union_type, '\\[.*\\]$', '')::VARCHAR AS all_null_union_type,
    c.case_without_else::VARCHAR AS case_without_else,
    c.decoded::VARCHAR AS decoded
  FROM computed AS c
  CROSS JOIN counted AS k
  CROSS JOIN union_typed AS u
  CROSS JOIN all_null_typed AS n
)

SELECT *
FROM final
