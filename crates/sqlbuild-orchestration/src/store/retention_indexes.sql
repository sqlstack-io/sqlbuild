CREATE INDEX IF NOT EXISTS sqb_runs_tick ON sqb_runs(tick_id);
CREATE INDEX IF NOT EXISTS sqb_ticks_expiry ON sqb_ticks(scheduled_for) WHERE outcome IN ('skipped', 'covered');
CREATE INDEX IF NOT EXISTS sqb_runs_snapshot ON sqb_runs(snapshot_id);
CREATE INDEX IF NOT EXISTS sqb_trigger_progress_snapshot ON sqb_trigger_progress(snapshot_id);
CREATE INDEX IF NOT EXISTS sqb_snapshot_nodes_hash ON sqb_snapshot_nodes(node_hash);
CREATE INDEX IF NOT EXISTS sqb_snapshot_sources_hash ON sqb_snapshot_sources(text_hash);
CREATE INDEX IF NOT EXISTS sqb_snapshot_documents_hash ON sqb_snapshot_documents(document_hash);
