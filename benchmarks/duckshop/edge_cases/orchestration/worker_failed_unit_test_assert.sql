TEST (name "stg_orders__amounts_stay_small");

WITH __ref__raw_orders AS (
  SELECT 1 AS order_id, 10 AS amount
),

__assert__amounts_stay_small AS (
  SELECT order_id FROM __ref("stg_orders") WHERE amount > 5
)

SELECT 1
