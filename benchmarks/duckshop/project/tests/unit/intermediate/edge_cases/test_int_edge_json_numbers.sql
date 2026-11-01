TEST (name "int_edge_json_numbers__parses_zero_fractions_and_rejects_two_decimal_points");

WITH __ref__stg_edge_amount_checks AS (
  SELECT
    v.check_id,
    NULL AS group_code,
    NULL::NUMBER(18, 0) AS narrow_key,
    NULL::NUMBER(38, 0) AS wide_key,
    NULL AS key_text,
    NULL::NUMBER(18, 2) AS amount,
    NULL::NUMBER(20, 15) AS fine_amount,
    NULL::FLOAT AS ratio,
    IFF(v.json_text = '@forty_zeros', '1.' || REPEAT('0', 40), v.json_text) AS json_text
  FROM VALUES
    ('AC-01', '{"amount": 2.0, "qty": 1.00}'),
    ('AC-02', '1.0.5'),
    ('AC-03', '{"amount": 1.00.25}'),
    ('AC-04', '[1.0.5, 2.0]'),
    ('AC-05', '12345678901234567890123456789012345678.0'),
    ('AC-06', '@forty_zeros'),
    ('AC-07', '{"amount": -0.0}'),
    ('AC-08', '{"amount": 2.0e5}'),
    ('AC-09', '{"amount": 100.00}'),
    ('AC-10', '123456789012345678901234567890123456789.0') AS v(check_id, json_text)
),

__expected__int_edge_json_numbers AS (
  SELECT
    NULL AS check_id,
    NULL AS parsed_kind,
    NULL AS parsed_text,
    NULL AS amount_kind,
    NULL AS amount_text,
    NULL AS is_valid_json
)
