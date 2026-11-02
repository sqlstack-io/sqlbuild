TEST (name "rpt_edge_case_array_model_keys__lists_one_key_per_array_model");

WITH __ref__int_edge_check_number_arrays AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_number-1') AS v(check_id)
),

__ref__int_edge_check_text_arrays AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_text_a-1') AS v(check_id)
),

__ref__int_edge_callback_wall_time_lists AS (
  SELECT
    v.merchant_zone::VARCHAR AS merchant_zone
  FROM VALUES
    ('callback_wal-1') AS v(merchant_zone)
),

__ref__int_edge_check_key_parts AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_key_pa-1') AS v(check_id)
),

__expected__rpt_edge_case_array_model_keys AS (
  SELECT
    NULL AS model_key_id,
    NULL AS model_name,
    NULL AS model_key
)
