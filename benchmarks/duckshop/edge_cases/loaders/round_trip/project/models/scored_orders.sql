MODEL (description "Orders with their partner score", materialized table);

SELECT o.order_id, s.score
FROM __ref("orders") AS o
JOIN __source("partner_scores") AS s ON o.order_id = s.order_id
