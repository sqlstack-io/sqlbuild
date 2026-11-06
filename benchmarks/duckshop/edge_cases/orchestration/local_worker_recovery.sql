-- A long-running statement for scheduler crash, adoption and cancellation checks.
-- The supervising test cancels this query; it is not part of benchmark measurements.
SELECT sum(sin(value)) AS amount FROM range(10000000000) t(value)
