TEST (name "stg_orders__doubles_amounts");

WITH __ref__raw_orders AS (
  SELECT 1 AS order_id, 10 AS amount
),

__expected__stg_orders AS (
  SELECT 1 AS order_id, 999 AS amount
)

SELECT 1
