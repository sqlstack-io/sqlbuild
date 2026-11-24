MODEL (description "Test model deprecated_orders.", tags [sqb_only, deprecated]);

select order_id from __ref("local_only")
