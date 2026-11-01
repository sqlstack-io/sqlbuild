<script lang="ts">
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import LoadRow from '$lib/components/list/load-row.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import ExplainAccess from '$lib/components/platform/explain-access.svelte';
	import GatedButton from '$lib/components/platform/gated-button.svelte';
	import NoAccess from '$lib/components/platform/no-access.svelte';
	import PageTabs from '$lib/components/platform/page-tabs.svelte';
	import Panel from '$lib/components/platform/panel.svelte';
	import type { ApprovalState, RoleApproval, RoleAssignment, RoleGrant, RoleSource } from '$lib/api';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { errorMessage, formatAssignmentScope, locationName } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';

	const canAssign = $derived(app.can('role.assign'));
	const guard = <T,>(load: () => Promise<T>, empty: T) => (app.can('role.assign') ? load() : Promise.resolve(empty));

	const approvals = createResource((client) => guard(() => client.listRoleApprovals(), []));
	const assignments = createResource((client) => guard(() => client.listRoleAssignments(), []));
	const roles = createResource((client, scope) => guard(() => client.listRoles(scope.locationId), []));
	const accounts = createResource((client) => (app.can('user.manage') ? client.listAccounts() : Promise.resolve([])));

	type Tab = 'approvals' | 'assignments' | 'roles' | 'explain';
	let tab = $state<Tab>('approvals');

	const pending = $derived((approvals.data ?? []).filter((a) => a.state === 'pending'));
	const decided = $derived((approvals.data ?? []).filter((a) => a.state !== 'pending'));
	const locations = $derived(app.bootstrap?.locations ?? []);

	const stateTone: Record<ApprovalState, string> = {
		pending: 'var(--sb-warning)',
		approved: 'var(--sb-success)',
		rejected: 'var(--sb-error)',
		superseded: 'var(--sb-text-faint)'
	};
	const sourceLabel: Record<RoleSource, string> = { built_in: 'Built in', server_config: 'Server config', project: 'Project code' };

	function grantLine(g: RoleGrant): string {
		return `nodes = [${g.nodes.map((n) => `"${n}"`).join(', ')}]  ${g.permissions.join(', ')}`;
	}

	// ---- approvals ----
	let notes = $state<Record<string, string>>({});
	let busy = $state<string | null>(null);
	let actionError = $state<string | null>(null);

	async function decide(approval: RoleApproval, decision: 'approve' | 'reject') {
		busy = approval.id;
		actionError = null;
		try {
			await app.client.decideRoleApproval(approval.id, decision, notes[approval.id] ?? '');
			approvals.refresh();
			roles.refresh();
			app.refreshNavCounts();
		} catch (error) {
			actionError = errorMessage(error);
		} finally {
			busy = null;
		}
	}

	/** Approved grants the proposal leaves untouched, shown as diff context. */
	function unchangedGrants(approval: RoleApproval): RoleGrant[] {
		const removed = new Set(approval.removed.map(grantLine));
		return (roles.data?.find((r) => r.name === approval.role)?.grants ?? []).filter((g) => !removed.has(grantLine(g)));
	}

	// ---- assignments ----
	let newPrincipal = $state('');
	let newRole = $state('');
	let newLocation = $state('');

	async function assign() {
		busy = 'assign';
		actionError = null;
		try {
			await app.client.createRoleAssignment({
				principal: newPrincipal.trim(),
				role: newRole,
				scope: { locationId: newLocation || null }
			});
			newPrincipal = '';
			assignments.refresh();
		} catch (error) {
			actionError = errorMessage(error);
		} finally {
			busy = null;
		}
	}

	const ownAdmin = $derived((assignments.data ?? []).filter((a) => a.principal.name === app.user?.name && a.role === 'admin' && !a.stale));
	const isOwnLastAdmin = (a: RoleAssignment) => ownAdmin.length === 1 && ownAdmin[0].id === a.id;

	async function remove(a: RoleAssignment) {
		const message = isOwnLastAdmin(a)
			? `This is YOUR LAST ADMIN ASSIGNMENT.\n\nRemoving it would immediately take away your access to users, roles, tokens, locations and the audit log, and nobody could undo it from this account. The server refuses this change; another admin has to make it.\n\nSend the request anyway?`
			: `Remove ${a.role} from ${a.principal.name} (${formatAssignmentScope(a.scope, locations)})?`;
		if (!confirm(message)) return;
		busy = a.id;
		actionError = null;
		try {
			await app.client.deleteRoleAssignment(a.id);
			assignments.refresh();
		} catch (error) {
			actionError = errorMessage(error);
		} finally {
			busy = null;
		}
	}

	const staleCount = $derived((assignments.data ?? []).filter((a) => a.stale).length);
</script>

<AppTopbar title="Roles & approvals">
	<Button
		variant="outline"
		size="sm"
		onclick={() => {
			approvals.refresh();
			assignments.refresh();
			roles.refresh();
		}}>↻ Refresh</Button
	>
</AppTopbar>

{#if !canAssign}
	<NoAccess what="roles and approvals" requires="role.assign" />
{:else}
	<PageTabs
		bind:value={tab}
		tabs={[
			{ value: 'approvals', label: 'Waiting for approval', count: pending.length },
			{ value: 'assignments', label: 'Who has which role', count: assignments.data?.length },
			{ value: 'roles', label: 'Roles', count: roles.data?.length },
			{ value: 'explain', label: 'Why can or can’t they?' }
		]}
	/>

	{#if actionError}<p class="err">{actionError}</p>{/if}

	<div class="min-h-0 flex-1 overflow-auto">
		{#if tab === 'approvals'}
			<div class="px-6 pt-5">
				<p class="lede">
					Roles defined in the project code are only proposals. Taking permissions away applies when the code is deployed. Giving more waits here: the approved version stays in force until an admin approves the change.
				</p>
				{#each pending as a (a.id)}
					<div class="approval">
						<div class="approval-head">
							<b>{a.role}</b>
							<span class="muted" title="Snapshot generation {a.snapshotGeneration}">{locationName(locations, a.locationId)} · version {a.snapshotGeneration} · commit <code>{a.gitRevision}</code> by {a.gitAuthor} · proposed {formatAgo(a.proposedAt, app.now)}</span>
						</div>
						<div class="diff" aria-label="Proposed change">
							{#each unchangedGrants(a) as g (grantLine(g))}
								<div class="line ctx"><span> </span>{grantLine(g)}</div>
							{/each}
							{#each a.removed as g (grantLine(g))}
								<div class="line del"><span>-</span>{grantLine(g)}</div>
							{/each}
							{#each a.added as g (grantLine(g))}
								<div class="line add"><span>+</span>{grantLine(g)}</div>
							{/each}
						</div>
						<div class="decide">
							<input class="field" placeholder="Note for the audit log (optional)" bind:value={() => notes[a.id] ?? '', (v) => (notes = { ...notes, [a.id]: v })} />
							<GatedButton requires="role.assign" variant="default" busy={busy === a.id} onclick={() => decide(a, 'approve')}>Approve</GatedButton>
							<GatedButton requires="role.assign" variant="destructive" busy={busy === a.id} onclick={() => decide(a, 'reject')}>Reject</GatedButton>
						</div>
					</div>
				{:else}
					<p class="muted mb-6">{approvals.data ? 'Nothing is waiting for approval.' : approvals.error ? `Couldn't load: ${approvals.error}` : 'Loading…'}</p>
				{/each}

				{#if decided.length}
					<Panel title="Already decided">
						<table class="sb-list static framed">
							<thead>
								<tr>
									<th>Role</th>
									<th>Change</th>
									<th>Decision</th>
									<th>Note</th>
								</tr>
							</thead>
							<tbody>
								{#each decided as a (a.id)}
									<tr>
										<td>
											<strong>{a.role}</strong>
											<div class="cell-desc">{locationName(locations, a.locationId)} · <code>{a.gitRevision}</code> by {a.gitAuthor}</div>
										</td>
										<td class="muted">
											{#each a.added as g (grantLine(g))}<div><code>+ {grantLine(g)}</code></div>{/each}
											{#each a.removed as g (grantLine(g))}<div><code>- {grantLine(g)}</code></div>{/each}
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
		{:else if tab === 'assignments'}
			<div class="create-bar">
				{#if accounts.data?.length}
					<select class="field" bind:value={newPrincipal} aria-label="Principal">
						<option value="">Person or service account…</option>
						{#each accounts.data as p (p.name)}<option value={p.name}>{p.name}</option>{/each}
					</select>
				{:else}
					<input class="field" placeholder="Person or service account" bind:value={newPrincipal} aria-label="Person or service account" />
				{/if}
				<select class="field" bind:value={newRole} aria-label="Role">
					<option value="">Role…</option>
					{#each roles.data ?? [] as r (r.name)}<option value={r.name}>{r.name}</option>{/each}
				</select>
				<select class="field" bind:value={newLocation} aria-label="Location">
					<option value="">All locations</option>
					{#each locations as l (l.id)}<option value={l.id}>{l.name} · {l.target.name}</option>{/each}
				</select>
				<GatedButton requires="role.assign" variant="default" busy={busy === 'assign'} disabled={!newPrincipal.trim() || !newRole} onclick={assign}>Assign role</GatedButton>
				{#if staleCount}<span class="note warn">{staleCount} stale {staleCount === 1 ? 'assignment grants' : 'assignments grant'} nothing</span>{/if}
			</div>
			<table class="sb-list static">
				<thead>
					<tr>
						<th>Who</th>
						<th>Role</th>
						<th>Where</th>
						<th>Given by</th>
						<th>Assigned</th>
						<th></th>
					</tr>
				</thead>
				<tbody>
					{#each assignments.data ?? [] as a (a.id)}
						<tr class:sb-row-off={a.stale}>
							<td>
								<strong>{a.principal.displayName}</strong>
								<div class="cell-desc"><code>{a.principal.name}</code></div>
							</td>
							<td>
								{a.role}
								{#if a.stale}<div class="cell-desc warn">This role no longer exists in the code, so it gives nothing</div>{/if}
								{#if isOwnLastAdmin(a)}<div class="cell-desc warn">Your last admin role: removing it locks you out of administration</div>{/if}
							</td>
							<td class="muted">{formatAssignmentScope(a.scope, locations)}</td>
							<td class="muted">{a.source === 'sso' ? `SSO group ${a.ssoGroup}` : 'Manual'}</td>
							<td>
								<RelativeTime value={formatAgo(a.assignedAt, app.now)} exact={formatStamp(a.assignedAt)} />
								{#if a.assignedBy}<div class="cell-desc">by {a.assignedBy.name}</div>{/if}
							</td>
							<td class="num">
								{#if a.source === 'manual'}
									<GatedButton requires="role.assign" size="xs" variant="destructive" busy={busy === a.id} onclick={() => remove(a)}>Remove</GatedButton>
								{:else}
									<span class="faint" title="Comes from their SSO group; change the group mapping in the server config">from their SSO group</span>
								{/if}
							</td>
						</tr>
					{:else}
						<LoadRow colspan={6} resource={assignments} empty="No one has a role yet. Give one above." />
					{/each}
				</tbody>
			</table>
		{:else if tab === 'roles'}
			<div class="px-6 pt-5">
				<p class="lede">
					Roles are defined in code (project roles, compiled into the snapshot) or the server config; they're read-only here. Admin actions can never be granted from project code.
				</p>
				<table class="sb-list static framed">
					<thead>
						<tr>
							<th>Role</th>
							<th>Source</th>
							<th>What it allows</th>
						</tr>
					</thead>
					<tbody>
						{#each roles.data ?? [] as r (r.name)}
							<tr>
								<td>
									<strong>{r.name}</strong>
									<div class="cell-desc">{r.description}</div>
									{#if r.pendingApprovalId}<div class="cell-desc warn">A change giving it more is waiting for approval</div>{/if}
								</td>
								<td class="muted">{sourceLabel[r.source]}{#if r.sourceFile}<div class="cell-desc"><code>{r.sourceFile}</code></div>{/if}</td>
								<td>
									{#each r.grants as g (grantLine(g))}
										<div class="grant"><code>{g.nodes.join(' ')}</code> <span class="muted">{g.permissions.join(', ')}</span></div>
									{/each}
								</td>
							</tr>
						{:else}
							<LoadRow colspan={3} resource={roles} empty="No roles." />
						{/each}
					</tbody>
				</table>
			</div>
		{:else}
			<div class="px-6 pt-5">
				<p class="lede">Why can or can't someone do something? The server shows which of their roles allow it, or which roles would.</p>
				<ExplainAccess principals={accounts.data ?? []} />
			</div>
		{/if}
	</div>
{/if}

<style>
	:global(.sb-list.static tbody tr) {
		cursor: default;
	}
	.framed {
		border-top: 1px solid var(--border);
	}
	.lede {
		margin-bottom: 18px;
		max-width: 860px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.muted {
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.faint {
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	.warn {
		color: var(--sb-warning) !important;
	}
	.err {
		padding: 10px 20px 0;
		color: var(--sb-error);
		font-size: 12.5px;
	}
	.note {
		margin-left: 8px;
		font-size: 12px;
	}
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.approval {
		margin-bottom: 18px;
		padding: 12px 14px;
		border: 1px solid color-mix(in srgb, var(--sb-warning) 35%, var(--border));
		border-radius: 2px;
		background: var(--card);
		font-size: 12.5px;
	}
	.approval-head {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 10px;
		margin-bottom: 10px;
	}
	.approval-head b {
		font-size: 13px;
		font-weight: 600;
	}
	.diff {
		padding: 6px 0;
		border-radius: 2px;
		background: var(--sb-inset);
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.line {
		padding: 1px 10px;
		white-space: pre;
	}
	.line span {
		display: inline-block;
		width: 16px;
		color: var(--sb-text-faint);
	}
	.line.ctx {
		color: var(--muted-foreground);
	}
	.line.add {
		background: color-mix(in srgb, var(--sb-success) 12%, transparent);
	}
	.line.del {
		background: color-mix(in srgb, var(--sb-error) 12%, transparent);
	}
	.decide {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-top: 10px;
	}
	.decide .field {
		flex: 1;
	}
	.create-bar {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 12px 20px;
		border-bottom: 1px solid var(--border);
	}
	.field {
		height: 32px;
		padding: 0 9px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		color: var(--foreground);
		font-size: 12.5px;
	}
	.grant + .grant {
		margin-top: 2px;
	}
</style>
