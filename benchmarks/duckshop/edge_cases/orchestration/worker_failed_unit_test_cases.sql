TEST (name "stg_orders__doubles_case_amounts", parameters (amount integer), cases (ten (amount 10), twenty (amount 20)));

WITH __ref__raw_orders AS (
  SELECT 1 AS order_id, @param("amount") AS amount
),

__expected__stg_orders AS (
  SELECT 1 AS order_id, 999 AS amount
)

SELECT 1
