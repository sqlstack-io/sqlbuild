-- When the worker read the lines of each output segment. Segments stored before this have none
-- and read as the time their batch arrived.
ALTER TABLE sqb_run_output_segments ADD COLUMN read_at {time};
