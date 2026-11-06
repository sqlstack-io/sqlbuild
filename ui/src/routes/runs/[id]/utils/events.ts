// Plain one-line summaries of run events; the raw type stays available as a tooltip.

import type { RunEvent } from '$lib/api';
import type { EventCategory } from '../types';

export const EVENT_CATEGORIES: { value: EventCategory; label: string }[] = [
	{ value: 'run', label: 'Run' },
	{ value: 'nodes', label: 'Nodes' },
	{ value: 'queries', label: 'Queries' },
	{ value: 'checks', label: 'Checks' }
];

export function eventCategory(e: RunEvent): EventCategory {
	if (e.type.startsWith('statement_')) return 'queries';
	if (e.type.startsWith('resource_attempt_')) return 'nodes';
	if (e.type === 'audit_completed') return 'checks';
	return 'run';
}

export function eventSummary(e: RunEvent): string {
	switch (e.type) {
		case 'requested':
			return e.message;
		case 'queued':
			return 'Waiting for its turn';
		case 'claimed':
			return 'Its turn came: the data it writes is reserved for this run';
		case 'launched':
			return 'Started a container for the run';
		case 'cancel_requested':
			return e.message;
		case 'presumed_failed':
			return 'Lost contact with the run; marked failed';
		case 'terminated':
			return `Finished${e.message.match(/^Terminated: (\w+)/) ? `: ${e.message.match(/^Terminated: (\w+)/)![1].replace('_', ' ')}` : ''}`;
		case 'invocation_started':
			return 'sqb started';
		case 'invocation_completed':
			return `sqb finished: ${e.message}`;
		case 'invocation_failed':
			return `sqb finished with failures: ${e.message}`;
		case 'resource_attempt_started':
			return 'Started';
		case 'resource_attempt_completed':
			return e.message;
		case 'resource_attempt_failed':
			return e.message.replace(/^[A-Za-z0-9_]+: /, 'Failed: ');
		case 'resource_attempt_skipped':
			return e.message;
		case 'statement_submitted':
			return 'Query sent to the warehouse';
		case 'statement_started':
			return 'Query running';
		case 'statement_completed':
			return `Query finished · ${e.message}`;
		case 'statement_failed':
			return `Query failed: ${e.message}`;
		case 'audit_completed':
			return `Checks: ${e.message}`;
	}
}
