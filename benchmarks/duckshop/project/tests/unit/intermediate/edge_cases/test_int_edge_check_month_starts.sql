TEST (name "int_edge_check_month_starts__mixes_dates_and_text_in_values");

WITH __ref__stg_edge_amount_checks AS (
  SELECT
    v.check_id::VARCHAR AS check_id,
    v.group_code::VARCHAR AS group_code,
    v.narrow_key::NUMBER(18, 0) AS narrow_key,
    v.wide_key::NUMBER(38, 0) AS wide_key,
    v.key_text::VARCHAR AS key_text,
    v.amount::NUMBER(18, 2) AS amount,
    v.ratio::FLOAT AS ratio
  FROM VALUES
    ('AC-01', 'G-1', 7, 7, '101,7', 10.05, 0.1),
    ('AC-02', 'G-1', 12, 12, '2026-03-04', -0.55, 0.2),
    ('AC-03', 'G-2', 3, NULL, '7', 0.00, 0.3),
    ('AC-04', 'G-2', NULL, 40, NULL, NULL, NULL) AS v(check_id, group_code, narrow_key, wide_key, key_text, amount, ratio)
),

__expected__int_edge_check_month_starts AS (
  SELECT
    NULL AS period_id,
    NULL AS period_start
)
