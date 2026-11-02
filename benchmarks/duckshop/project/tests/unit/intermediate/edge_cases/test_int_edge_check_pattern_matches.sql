TEST (name "int_edge_check_pattern_matches__matches_keys_against_group_patterns");

WITH __ref__stg_edge_amount_checks AS (
  SELECT
    v.check_id::VARCHAR AS check_id,
    v.group_code::VARCHAR AS group_code,
    v.key_text::VARCHAR AS key_text
  FROM VALUES
    ('AC-01', '[[:digit:]]{3}', '101,7'),
    ('AC-02', '^7$', '7'),
    ('AC-03', '[[:alpha:]]+', '7') AS v(check_id, group_code, key_text)
),

__expected__int_edge_check_pattern_matches AS (
  SELECT
    'AC-01' AS check_id,
    FALSE AS matches_group
  UNION ALL
  SELECT
    'AC-02' AS check_id,
    TRUE AS matches_group
  UNION ALL
  SELECT
    'AC-03' AS check_id,
    FALSE AS matches_group
)
