<script lang="ts">
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import PermissionName from '$lib/components/admin/permission-name.svelte';
	import type { ApprovalState } from '$lib/api';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { roleHref } from '$lib/platform/access';
	import { locationName } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import ApprovalCard from './approval-card.svelte';
	import type { RolesState } from './state.svelte';

	let { rs }: { rs: RolesState } = $props();
	const locations = $derived(app.bootstrap?.locations ?? []);

	const stateTone: Record<ApprovalState, string> = {
		pending: 'var(--sb-warning)',
		approved: 'var(--sb-success)',
		rejected: 'var(--sb-error)',
		superseded: 'var(--sb-text-faint)'
	};
</script>

<div class="px-6 pt-5 pb-8">
	<p class="lede">
		Roles in project code are proposals. Taking actions away applies as soon as the code is deployed; giving more waits here, and the role keeps its approved actions until an admin
		approves. Check who gains what before approving. You can't approve a change you committed.
	</p>
	{#each rs.pending as a (a.id)}
		<ApprovalCard approval={a} {rs} />
	{:else}
		<p class="muted mb-6">{rs.approvals.data ? 'Nothing is waiting for approval.' : rs.approvals.error ? `Couldn't load: ${rs.approvals.error}` : 'Loading…'}</p>
	{/each}

	{#if rs.decided.length}
		<Panel title="Decided">
			<table class="sb-list static framed">
				<thead>
					<tr>
						<th>Role</th>
						<th>Change</th>
						<th>Decision</th>
						<th>Comment</th>
					</tr>
				</thead>
				<tbody>
					{#each rs.decided as a (a.id)}
						<tr>
							<td>
								<a class="link" href={roleHref(a.role)}>{a.role}</a>
								<div class="cell-desc">{locationName(locations, a.locationId)} · <code>{a.gitRevision}</code> by {a.gitAuthor}</div>
							</td>
							<td class="muted">
								{#each a.added as g (g.nodes.join('|') + g.permissions.join('|'))}
									{#each g.permissions as p (p)}<div>+ <PermissionName action={p} /> on <code>{g.nodes.join(', ')}</code></div>{/each}
								{/each}
								{#each a.removed as g (g.nodes.join('|') + g.permissions.join('|'))}
									{#each g.permissions as p (p)}<div>− <PermissionName action={p} /> on <code>{g.nodes.join(', ')}</code></div>{/each}
								{/each}
							</td>
							<td>
								<StatusLabel tone={stateTone[a.state]} label={a.state[0].toUpperCase() + a.state.slice(1)} />
								<div class="cell-desc" title={formatStamp(a.decidedAt)}>{a.decidedBy?.name ?? '—'} · {formatAgo(a.decidedAt, app.now)}</div>
							</td>
							<td class="muted">{a.note ?? '—'}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</Panel>
	{/if}
</div>

<style>
	:global(.sb-list.static tbody tr) {
		cursor: default;
	}
	.lede {
		max-width: 860px;
		margin-bottom: 18px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.framed {
		border-top: 1px solid var(--border);
	}
	.muted {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
</style>
