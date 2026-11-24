MODEL (
  description """
    One row per category node with units and net sales of its own SKUs and of its whole subtree,
    using the slug path to roll leaf sales up to every ancestor.
  """,
  columns (
    category_id (type VARCHAR, nullable false, audits [not_null, unique]),
    slug_path (type VARCHAR),
    depth (type INT),
    is_leaf (type BOOLEAN),
    own_units (type INT),
    subtree_units (type INT),
    subtree_net_amount_minor (type INT),
    share_of_parent_units (type NUMBER(9, 4)),
  ),
);

WITH categories AS (
  SELECT *
  FROM __ref("int_category_paths")
),

catalogue AS (
  SELECT *
  FROM __ref("dim_products")
),

lines AS (
  SELECT *
  FROM __ref("fct_order_lines")
),

product_catalogue AS (
  SELECT *
  FROM __ref("int_product_catalogue")
),

product_categories AS (
  SELECT
    p.sku,
    c.category_id,
    c.slug_path
  FROM catalogue AS p
  INNER JOIN product_catalogue AS pc
    ON p.sku = pc.sku
  INNER JOIN categories AS c
    ON pc.category_id = c.category_id
),

node_ranges AS (
  SELECT
    c.category_id,
    c.slug_path || '/' AS range_start,
    c.slug_path || '0' AS range_end
  FROM categories AS c
),

own_sales AS (
  SELECT
    pc.category_id,
    pc.slug_path || '/' AS slug_path_key,
    SUM(l.quantity) AS own_units,
    SUM(l.net_amount_minor) AS own_net_minor
  FROM lines AS l
  INNER JOIN product_categories AS pc
    ON l.sku = pc.sku
  GROUP BY
    pc.category_id,
    pc.slug_path
),

subtree AS (
  SELECT
    r.category_id,
    SUM(s.own_units) AS subtree_units,
    SUM(s.own_net_minor) AS subtree_net_minor
  FROM node_ranges AS r
  INNER JOIN own_sales AS s
    ON s.slug_path_key >= r.range_start
    AND s.slug_path_key < r.range_end
  GROUP BY
    r.category_id
),

final AS (
  SELECT
    c.category_id,
    c.slug_path,
    c.depth,
    c.is_leaf,
    COALESCE(o.own_units, 0)::INT AS own_units,
    COALESCE(t.subtree_units, 0)::INT AS subtree_units,
    COALESCE(t.subtree_net_minor, 0)::INT AS subtree_net_amount_minor,
    ROUND(t.subtree_units / NULLIF(pt.subtree_units, 0), 4)::NUMBER(9, 4) AS share_of_parent_units
  FROM categories AS c
  LEFT JOIN own_sales AS o
    ON c.category_id = o.category_id
  LEFT JOIN subtree AS t
    ON c.category_id = t.category_id
  LEFT JOIN subtree AS pt
    ON c.parent_category_id = pt.category_id
)

SELECT *
FROM final
