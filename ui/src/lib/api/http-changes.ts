import type { components } from '$control/schema.d.ts';
import type { ChangeOp, NodeChange, RoleChange, SnapshotChange } from './types/build';
import type { NodeKind, Run } from './types/operate';

type Wire = components['schemas'];

/** How many runs of each deploy the feed lists. */
const FOLLOWING_RUNS = 8;

const NODE_KINDS: Record<string, NodeKind> = { seed: 'seed', source: 'source' };

function opOf(kind: string): ChangeOp {
	return kind === 'added' || kind === 'removed' ? kind : 'changed';
}

function nodeChange(change: Wire['PublishedChange']): NodeChange {
	const prefix = change.subject_key.split(':', 1)[0];
	return {
		key: change.subject_key,
		kind: NODE_KINDS[prefix] ?? null,
		op: opOf(change.change_kind),
		formerKey: null,
		compiledOnly: false,
		detail: '',
		columns: []
	};
}

function roleChange(change: Wire['PublishedChange']): RoleChange {
	return { role: change.subject_key.slice('role:'.length), op: opOf(change.change_kind), detail: '', coverage: null };
}

/**
 * Publications newest first, as the change feed shows them. The server records which nodes and
 * roles changed, not job, trigger, column or cost changes, so those stay empty. Each deploy's runs
 * are the listed runs pinned to its snapshot, oldest first.
 */
export function toSnapshotChanges(wire: Wire['PublishedSnapshot'][], runs: Run[]): SnapshotChange[] {
	return wire.map((snapshot, index): SnapshotChange => {
		const previous = wire[index + 1] ?? null;
		const environmentChanged = previous !== null && previous.environment_hash !== snapshot.environment_hash;
		return {
			snapshotId: snapshot.snapshot_id,
			generation: snapshot.generation,
			previousGeneration: previous?.generation ?? null,
			gitRevision: snapshot.git_revision ?? null,
			gitDirty: snapshot.git_dirty,
			gitAuthor: snapshot.git_author ?? null,
			publishedAt: snapshot.published_at,
			publishedBy: null,
			nodes: snapshot.changes.filter((c) => !c.subject_key.startsWith('role:')).map(nodeChange),
			orchestration: [],
			roles: snapshot.changes.filter((c) => c.subject_key.startsWith('role:')).map(roleChange),
			environment: environmentChanged
				? { before: previous.environment_hash.slice(0, 12), after: snapshot.environment_hash.slice(0, 12), detail: 'Dependencies or the sqb version changed' }
				: null,
			followingRuns: runs
				.filter((r) => r.snapshotId === snapshot.snapshot_id && !r.parentRunId)
				.reverse()
				.slice(0, FOLLOWING_RUNS)
				.map(({ id, jobKey, kind, status, startedAt, queuedAt }) => ({ id, jobKey, kind, status, startedAt, queuedAt })),
			costChanges: null
		};
	});
}
