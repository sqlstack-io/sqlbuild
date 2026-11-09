<script lang="ts">
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import GatedButton from '$lib/components/platform/gated-button.svelte';
	import type { Principal, RoleAssignment, RoleDefinition } from '$lib/api';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { lockedHint, rankAssignments, roleHref, roleSummary, scopeWords } from '$lib/platform/access';
	import { errorMessage } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import SourceChip from './source-chip.svelte';

	type Props = {
		principal: Principal;
		/** Every assignment; filtered to `principal` here. */
		assignments: RoleAssignment[];
		/** Roles per location ID, for the add control and summaries. */
		rolesByLocation: Map<string, RoleDefinition[]>;
		onchange: () => void;
	};
	let { principal, assignments, rolesByLocation, onchange }: Props = $props();

	const locations = $derived(app.bootstrap?.locations ?? []);
	const allRoles = $derived([...new Map([...rolesByLocation.values()].flat().map((r) => [r.name, r])).values()]);
	const mine = $derived(rankAssignments(assignments.filter((a) => a.principal.name === principal.name), allRoles));
	const ownAdmin = $derived(assignments.filter((a) => a.principal.name === app.user?.name && a.role === 'admin' && !a.stale));
	const isOwnLastAdmin = (a: RoleAssignment) => ownAdmin.length === 1 && ownAdmin[0].id === a.id;

	let busy = $state<string | null>(null);
	let error = $state<string | null>(null);

	let newLocation = $state('');
	let newRole = $state('');
	/**
	 * Everywhere, only built-in roles, since a custom role belongs to the location that publishes it.
	 * In one location, its roles except admin, which is only given for every location.
	 */
	const choices = $derived.by(() => {
		if (newLocation) return (rolesByLocation.get(newLocation) ?? []).filter((r) => r.name !== 'admin');
		const lists = [...rolesByLocation.values()];
		if (!lists.length) return [];
		return lists[0].filter((r) => r.source === 'built_in' && lists.every((l) => l.some((x) => x.name === r.name)));
	});
	const roleOf = (name: string) => allRoles.find((r) => r.name === name);
	// A role picked for another place may not be on offer here.
	$effect(() => {
		if (newRole && !choices.some((r) => r.name === newRole)) newRole = '';
	});

	async function act(key: string, action: () => Promise<unknown>) {
		busy = key;
		error = null;
		try {
			await action();
			onchange();
			return true;
		} catch (e) {
			error = errorMessage(e);
			return false;
		} finally {
			busy = null;
		}
	}

	async function add() {
		const ok = await act('add', () => app.client.createRoleAssignment({ principal: principal.name, role: newRole, scope: { locationId: newLocation || null } }));
		if (ok) newRole = '';
	}

	function remove(a: RoleAssignment) {
		const where = scopeWords(a.scope.locationId, locations);
		const message = isOwnLastAdmin(a)
			? `This is YOUR LAST ADMIN ROLE.\n\nRemoving it would immediately take away your access to people, roles, tokens, locations and the audit log, and nobody could undo it from this account. The server refuses this change; another admin has to make it.\n\nSend the request anyway?`
			: `Remove ${a.role} from ${principal.displayName} in ${where}?`;
		if (!confirm(message)) return;
		void act(a.id, () => app.client.deleteRoleAssignment(a.id));
	}
</script>

<div class="assignments">
	<table class="sb-list static framed">
		<thead>
			<tr>
				<th>Role</th>
				<th>Where</th>
				<th>From</th>
				<th>Since</th>
				<th></th>
			</tr>
		</thead>
		<tbody>
			{#each mine as a (a.id)}
				{@const role = roleOf(a.role)}
				{@const hint = lockedHint(a)}
				<tr class:sb-row-off={a.stale}>
					<td>
						{#if role}<a class="link" href={roleHref(a.role)}>{a.role}</a>{:else}<span>{a.role}</span>{/if}
						{#if role}<div class="cell-desc">{roleSummary(role)}</div>{/if}
						{#if a.stale}<div class="cell-desc warn">This role no longer exists in the code, so it gives nothing</div>{/if}
						{#if isOwnLastAdmin(a)}<div class="cell-desc warn"><TriangleAlertIcon size={11} /> Your last admin role: removing it locks you out of administration</div>{/if}
					</td>
					<td class="muted">{scopeWords(a.scope.locationId, locations)}</td>
					<td><SourceChip assignment={a} /></td>
					<td>
						<RelativeTime value={formatAgo(a.assignedAt, app.now)} exact={formatStamp(a.assignedAt)} />
						{#if a.assignedBy}<div class="cell-desc">by {a.assignedBy.name}</div>{/if}
					</td>
					<td class="num">
						{#if a.source === 'manual'}
							<GatedButton requires="role.assign" size="xs" variant="destructive" busy={busy === a.id} onclick={() => remove(a)}>Remove</GatedButton>
						{:else if a.source === 'sso'}
							<a class="locked" href="/admin/roles?tab=sso" title={hint}>See SSO mapping</a>
						{:else}
							<span class="locked" title={hint}>{a.source === 'policy' ? `Change in ${a.definedIn}` : 'Default role'}</span>
						{/if}
					</td>
				</tr>
			{:else}
				<tr><td colspan="5" class="empty">No roles yet.</td></tr>
			{/each}
		</tbody>
	</table>

	{#if app.can('role.assign')}
		<div class="add">
			<select class="field" bind:value={newLocation} aria-label="Where">
				<option value="">Every location</option>
				{#each locations as l (l.id)}<option value={l.id}>{l.name}</option>{/each}
			</select>
			<select class="field" bind:value={newRole} aria-label="Role">
				<option value="">Role…</option>
				{#each choices as r (r.name)}<option value={r.name}>{r.name} · {roleSummary(r)}</option>{/each}
			</select>
			<GatedButton requires="role.assign" variant="default" busy={busy === 'add'} disabled={!newRole} title="Pick a role" onclick={add}>Give role</GatedButton>
		</div>
	{/if}
	{#if error}<p class="err">{error}</p>{/if}
</div>

<style>
	:global(.sb-list.static tbody tr) {
		cursor: default;
	}
	.framed {
		border-top: 1px solid var(--border);
	}
	.warn {
		display: flex;
		align-items: center;
		gap: 4px;
		color: var(--sb-warning) !important;
	}
	.locked {
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	a.locked:hover {
		color: var(--primary);
	}
	.add {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
		padding: 12px 0 0;
	}
	.field {
		height: 30px;
		padding: 0 8px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		color: var(--foreground);
		font-size: 12.5px;
	}
	.err {
		margin-top: 8px;
		color: var(--sb-error);
		font-size: 12.5px;
	}
</style>
