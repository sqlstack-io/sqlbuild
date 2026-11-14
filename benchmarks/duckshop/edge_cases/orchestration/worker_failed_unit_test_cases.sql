TEST (name "stg_orders__doubles_case_amounts", parameters (amount integer, doubled integer), cases (ten (amount 10, doubled 999), twenty (amount 20, doubled 40)));

WITH __ref__raw_orders AS (
  SELECT 1 AS order_id, @param("amount") AS amount
),

__expected__stg_orders AS (
  SELECT 1 AS order_id, @param("doubled") AS amount
)

SELECT 1
