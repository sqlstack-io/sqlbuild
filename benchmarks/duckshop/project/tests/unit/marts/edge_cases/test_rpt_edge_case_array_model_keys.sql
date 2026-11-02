TEST (name "rpt_edge_case_array_model_keys__lists_one_key_per_array_model");

WITH __ref__int_edge_callback_wall_time_lists AS (
  SELECT
    v.merchant_zone::VARCHAR AS merchant_zone
  FROM VALUES
    ('callback_wal-1') AS v(merchant_zone)
),

__expected__rpt_edge_case_array_model_keys AS (
  SELECT
    NULL AS model_key_id,
    NULL AS model_name,
    NULL AS model_key
)
