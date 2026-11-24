MODEL (description "Customer count read only through a SQL function whose body reads an undeclared raw table.", materialized table);
SELECT __udf("udf__raw_customer_count")(1) AS customer_count
