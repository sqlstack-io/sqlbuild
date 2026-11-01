CREATE INDEX sqb_node_finished_cursor ON recall_events(storage_id)
WHERE producer='scheduler' AND event_type='run_node_finished';
CREATE INDEX sqb_node_started_identity ON recall_events(
    invocation_id, (payload -> 'change' ->> 'claim_id'),
    (payload -> 'change' -> 'change' ->> 'node_key'), storage_id
) WHERE producer='scheduler' AND event_type='run_node_started';
