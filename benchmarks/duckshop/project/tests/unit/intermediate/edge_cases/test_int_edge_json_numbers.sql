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
    'AC-01' AS check_id,
    'OBJECT' AS parsed_kind,
    '{"amount":2,"qty":1}' AS parsed_text,
    'INTEGER' AS amount_kind,
    '2' AS amount_text,
    TRUE AS is_valid_json
  UNION ALL
  SELECT
    'AC-02' AS check_id,
    CAST(NULL AS VARCHAR) AS parsed_kind,
    CAST(NULL AS VARCHAR) AS parsed_text,
    CAST(NULL AS VARCHAR) AS amount_kind,
    CAST(NULL AS VARCHAR) AS amount_text,
    FALSE AS is_valid_json
  UNION ALL
  SELECT
    'AC-03' AS check_id,
    CAST(NULL AS VARCHAR) AS parsed_kind,
    CAST(NULL AS VARCHAR) AS parsed_text,
    CAST(NULL AS VARCHAR) AS amount_kind,
    CAST(NULL AS VARCHAR) AS amount_text,
    FALSE AS is_valid_json
  UNION ALL
  SELECT
    'AC-04' AS check_id,
    CAST(NULL AS VARCHAR) AS parsed_kind,
    CAST(NULL AS VARCHAR) AS parsed_text,
    CAST(NULL AS VARCHAR) AS amount_kind,
    CAST(NULL AS VARCHAR) AS amount_text,
    FALSE AS is_valid_json
  UNION ALL
  SELECT
    'AC-05' AS check_id,
    'INTEGER' AS parsed_kind,
    '12345678901234567890123456789012345678' AS parsed_text,
    CAST(NULL AS VARCHAR) AS amount_kind,
    CAST(NULL AS VARCHAR) AS amount_text,
    TRUE AS is_valid_json
  UNION ALL
  SELECT
    'AC-06' AS check_id,
    'INTEGER' AS parsed_kind,
    '1' AS parsed_text,
    CAST(NULL AS VARCHAR) AS amount_kind,
    CAST(NULL AS VARCHAR) AS amount_text,
    TRUE AS is_valid_json
  UNION ALL
  SELECT
    'AC-07' AS check_id,
    'OBJECT' AS parsed_kind,
    '{"amount":0}' AS parsed_text,
    'INTEGER' AS amount_kind,
    '0' AS amount_text,
    TRUE AS is_valid_json
  UNION ALL
  SELECT
    'AC-08' AS check_id,
    'OBJECT' AS parsed_kind,
    '{"amount":2.000000000000000e+05}' AS parsed_text,
    'DOUBLE' AS amount_kind,
    '200000' AS amount_text,
    TRUE AS is_valid_json
  UNION ALL
  SELECT
    'AC-09' AS check_id,
    'OBJECT' AS parsed_kind,
    '{"amount":100}' AS parsed_text,
    'INTEGER' AS amount_kind,
    '100' AS amount_text,
    TRUE AS is_valid_json
  UNION ALL
  SELECT
    'AC-10' AS check_id,
    'INTEGER' AS parsed_kind,
    '123456789012345678901234567890123456789' AS parsed_text,
    CAST(NULL AS VARCHAR) AS amount_kind,
    CAST(NULL AS VARCHAR) AS amount_text,
    TRUE AS is_valid_json
)
