TEST (name "int_edge_check_pattern_matches__matches_keys_against_group_patterns");

WITH __ref__stg_edge_amount_checks AS (
  SELECT
    v.check_id::VARCHAR AS check_id,
    v.group_code::VARCHAR AS group_code,
    v.key_text::VARCHAR AS key_text
  FROM VALUES
    ('AC-01', '^1(?=0)', '101,7'),
    ('AC-02', '[[:digit:]]+', '7') AS v(check_id, group_code, key_text)
),

__expected__int_edge_check_pattern_matches AS (
  SELECT
    NULL AS check_id,
    NULL AS matches_group
)
