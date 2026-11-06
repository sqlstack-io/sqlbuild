TEST (name "rpt_edge_scheduled_function__advances_the_inherited_function_quantity");

WITH __seed__edge_quantity_steps AS (
  SELECT v.quantity
  FROM VALUES
    (-2),
    (23) AS v(quantity)
),

__expected__rpt_edge_scheduled_function AS (
  SELECT v.quantity
  FROM VALUES
    (-1),
    (24) AS v(quantity)
)
