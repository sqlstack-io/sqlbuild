CREATE INDEX IF NOT EXISTS sqb_runs_tick ON sqb_runs(tick_id);
CREATE INDEX IF NOT EXISTS sqb_ticks_expiry ON sqb_ticks(scheduled_for) WHERE outcome IN ('skipped', 'covered');
