MODEL (
  description """
    One row per reconciliation check with numeric reads at their edges: FLOAT ratios and amounts
    floored with and without a scale, BITAND of keys, ZEROIFNULL, ::BIGINT and ::DOUBLE casts, keys
    compared with IS NOT DISTINCT FROM and EQUAL_NULL, a comma join to the group list written after
    an explicit join, and HASH_AGG of a group read in two row orders, which Snowflake does not let
    change the hash
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    ratio_floor (type FLOAT),
    amount_floor_tenths (type NUMBER(38, 1)),
    narrow_key_bits (type NUMBER(38, 0)),
    amount_or_zero (type NUMBER(18, 2)),
    narrow_key_bigint (type NUMBER(38, 0)),
    amount_double (type DOUBLE),
    keys_not_distinct (type BOOLEAN),
    keys_equal_null (type BOOLEAN),
    group_name (type VARCHAR),
    group_check_count (type NUMBER(38, 0)),
    group_hash_is_order_free (type BOOLEAN),
  ),
);

WITH checks AS (
  SELECT
    c.check_id,
    c.group_code,
    c.narrow_key,
    c.wide_key,
    c.amount,
    c.ratio
  FROM __ref("stg_edge_amount_checks") AS c
),

group_names AS (
  SELECT
    v.group_code,
    v.group_name
  FROM VALUES
    ('G-1', 'Mallard desk'),
    ('G-2', 'Teal desk'),
    ('G-3', 'Eider desk') AS v(group_code, group_name)
),

group_sizes AS (
  SELECT
    c.group_code,
    COUNT(*) AS group_check_count
  FROM checks AS c
  GROUP BY
    c.group_code
),

checks_forward AS (
  SELECT
    k.group_code,
    k.check_id,
    k.amount
  FROM checks AS k
  ORDER BY
    k.check_id
),

checks_backward AS (
  SELECT
    k.group_code,
    k.check_id,
    k.amount
  FROM checks AS k
  ORDER BY
    k.check_id DESC
),

hash_forward AS (
  SELECT
    c.group_code,
    HASH_AGG(c.check_id, c.amount) AS group_hash
  FROM checks_forward AS c
  GROUP BY
    c.group_code
),

hash_backward AS (
  SELECT
    c.group_code,
    HASH_AGG(c.check_id, c.amount) AS group_hash
  FROM checks_backward AS c
  GROUP BY
    c.group_code
),

final AS (
  SELECT
    c.check_id,
    FLOOR(c.ratio)::FLOAT AS ratio_floor,
    FLOOR(c.amount, 1)::NUMBER(38, 1) AS amount_floor_tenths,
    BITAND(c.narrow_key, 6)::NUMBER(38, 0) AS narrow_key_bits,
    ZEROIFNULL(c.amount)::NUMBER(18, 2) AS amount_or_zero,
    c.narrow_key::BIGINT AS narrow_key_bigint,
    c.amount::DOUBLE AS amount_double,
    (c.narrow_key IS NOT DISTINCT FROM c.wide_key)::BOOLEAN AS keys_not_distinct,
    EQUAL_NULL(c.narrow_key, c.wide_key)::BOOLEAN AS keys_equal_null,
    n.group_name::VARCHAR AS group_name,
    s.group_check_count::NUMBER(38, 0) AS group_check_count,
    (f.group_hash = b.group_hash)::BOOLEAN AS group_hash_is_order_free
  FROM checks AS c
  INNER JOIN group_sizes AS s
    ON c.group_code = s.group_code,
  group_names AS n,
  hash_forward AS f,
  hash_backward AS b
  WHERE
    n.group_code = c.group_code
    AND f.group_code = c.group_code
    AND b.group_code = c.group_code
)

SELECT *
FROM final
