MODEL (description "Customers read only through a table function whose body reads an undeclared raw table.", materialized table);
SELECT k.customer_id
FROM __table_fn("table_fn__raw_customers_from")(1) AS k
