import { describe, expect, it } from 'vitest';
import type { components } from '$control/schema.d.ts';
import { toSnapshotChanges } from './http-changes';
import type { Run } from './types/operate';

type W = components['schemas'];

function snapshot(id: string, generation: number, environment: string, changes: [string, string][]): W['PublishedSnapshot'] {
	return {
		snapshot_id: id,
		generation,
		git_revision: generation === 2 ? 'a1b2c3d' : null,
		git_dirty: generation === 2,
		git_author: generation === 2 ? 'Avery Example' : null,
		environment_hash: environment,
		sqb_version: '0.200.0',
		published_at: `2026-11-0${generation}T09:00:00Z`,
		changes: changes.map(([change_kind, subject_key]) => ({ change_kind, subject_key }))
	};
}

function run(id: string, snapshotId: string, queuedAt: string, parentRunId: string | null = null): Run {
	return { id, snapshotId, parentRunId, jobKey: 'orders', kind: 'job', status: 'success', startedAt: queuedAt, queuedAt } as Run;
}

describe('change feed mapping', () => {
	const newest = snapshot('s-2', 2, 'env-bbbbbbbbbbbbbbbb', [
		['removed', 'model:stock'],
		['changed', 'seed:regions'],
		['added', 'role:analyst']
	]);
	const oldest = snapshot('s-1', 1, 'env-aaaaaaaaaaaaaaaa', [['added', 'model:orders']]);
	// Runs are listed newest first, as the server returns them.
	const runs = [run('r-late', 's-2', '2026-11-02T11:00:00Z'), run('r-child', 's-2', '2026-11-02T10:30:00Z', 'r-early'), run('r-early', 's-2', '2026-11-02T10:00:00Z'), run('r-old', 's-1', '2026-11-01T10:00:00Z')];
	const [second, first] = toSnapshotChanges([newest, oldest], runs);

	it('splits roles from nodes and keeps each change kind', () => {
		expect(second.nodes.map((n) => [n.op, n.key, n.kind])).toEqual([
			['removed', 'model:stock', null],
			['changed', 'seed:regions', 'seed']
		]);
		expect(second.roles.map((r) => [r.op, r.role])).toEqual([['added', 'analyst']]);
	});

	it('reports an environment change only against the previous publication', () => {
		expect(second.environment).toMatchObject({ before: 'env-aaaaaaaa', after: 'env-bbbbbbbb' });
		expect(second.previousGeneration).toBe(1);
		expect(first.environment).toBeNull();
		expect(first.previousGeneration).toBeNull();
	});

	it("lists each deploy's own top-level runs, oldest first", () => {
		expect(second.followingRuns.map((r) => r.id)).toEqual(['r-early', 'r-late']);
		expect(first.followingRuns.map((r) => r.id)).toEqual(['r-old']);
	});

	it('keeps missing git facts empty instead of inventing them', () => {
		expect([second.gitRevision, second.gitAuthor, second.gitDirty]).toEqual(['a1b2c3d', 'Avery Example', true]);
		expect([first.gitRevision, first.gitAuthor, first.publishedBy]).toEqual([null, null, null]);
	});
});
