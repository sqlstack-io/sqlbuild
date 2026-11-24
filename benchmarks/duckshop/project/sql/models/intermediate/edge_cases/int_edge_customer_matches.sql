MODEL (
  description """
    One row per customer comparing the current and previous name for identity matching, by edit
    distance and Jaro-Winkler similarity over characters, not bytes, and ranking partner order
    numbers exactly and as FLOAT, where numbers past 2^53 tie
  """,
  tags [edge_case],
  columns (
    customer_id (type VARCHAR, nullable false, audits [not_null, unique]),
    name_edit_distance (type INT),
    name_similarity (type INT),
    is_same_person (type BOOLEAN),
    order_number_rank (type INT),
    order_number_float_rank (type INT),
  ),
);

WITH profiles AS (
  SELECT
    p.customer_id,
    p.full_name,
    p.previous_full_name,
    p.partner_order_number
  FROM __ref("stg_edge_customer_profiles") AS p
),

final AS (
  SELECT
    p.customer_id,
    EDITDISTANCE(LOWER(p.full_name), LOWER(p.previous_full_name))::INT AS name_edit_distance,
    JAROWINKLER_SIMILARITY(p.full_name, p.previous_full_name)::INT AS name_similarity,
    COALESCE(
      JAROWINKLER_SIMILARITY(p.full_name, p.previous_full_name) >= 90,
      FALSE
    ) AS is_same_person,
    DENSE_RANK() OVER (ORDER BY p.partner_order_number DESC)::INT AS order_number_rank,
    DENSE_RANK() OVER (ORDER BY p.partner_order_number::FLOAT DESC)::INT AS order_number_float_rank
  FROM profiles AS p
)

SELECT *
FROM final
