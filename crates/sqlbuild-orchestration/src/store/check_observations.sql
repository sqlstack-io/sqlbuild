CREATE INDEX sqb_observed_node_cursor ON recall_events(storage_id)
WHERE producer='scheduler' AND event_type IN ('run_node_finished','run_check_finished');
CREATE INDEX sqb_observed_node_identity ON recall_events(
    invocation_id, (payload -> 'change' -> 'change' ->> 'node_key'),
    (payload -> 'change' ->> 'claim_id'), storage_id DESC
) WHERE producer='scheduler';
CREATE INDEX sqb_observed_node_decisive ON recall_events(
    (payload -> 'change' -> 'change' ->> 'node_key'), storage_id DESC
) WHERE producer='scheduler' AND event_type IN ('run_node_finished','run_check_finished')
AND (payload -> 'change' -> 'change' ->> 'outcome') IN ('success','failed');
