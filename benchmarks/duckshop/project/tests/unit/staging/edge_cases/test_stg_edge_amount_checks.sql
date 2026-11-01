TEST (name "stg_edge_amount_checks__keeps_key_widths_and_scales");

WITH __source__edge__amount_checks AS (
  SELECT
    v.check_id,
    v.group_code,
    v.narrow_key::NUMBER(18, 0) AS narrow_key,
    v.wide_key::NUMBER(38, 0) AS wide_key,
    v.key_text,
    v.amount::NUMBER(18, 2) AS amount,
    v.fine_amount::NUMBER(20, 15) AS fine_amount,
    v.ratio::FLOAT AS ratio,
    v.json_text,
    '2026-03-06 00:00:00'::TIMESTAMP_NTZ AS _loaded_at
  FROM VALUES
    ('AC-01', 'G-1', 7, '12345678901234567890123', '01', 10.00, 0.123456789012345, 0.29, '1.0'),
    ('AC-02', 'G-2', NULL, NULL, NULL, NULL, NULL, NULL, NULL) AS v(check_id, group_code, narrow_key, wide_key, key_text, amount, fine_amount, ratio, json_text)
),

__expected__stg_edge_amount_checks AS (
  SELECT
    NULL AS check_id,
    NULL AS narrow_key,
    NULL AS wide_key,
    NULL AS amount,
    NULL AS fine_amount
)
