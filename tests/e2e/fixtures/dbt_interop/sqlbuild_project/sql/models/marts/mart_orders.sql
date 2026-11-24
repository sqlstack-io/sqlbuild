MODEL (description "Test model mart_orders.", tags [marts]);

select order_id from __ref("downstream_orders")
