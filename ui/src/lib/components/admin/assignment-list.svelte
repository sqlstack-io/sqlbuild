<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import { to } from '$lib/paths';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import EllipsisIcon from '@lucide/svelte/icons/ellipsis';
	import GatedButton from '$lib/components/platform/gated-button.svelte';
	import ConfirmDialog from '$lib/components/platform/confirm-dialog.svelte';
	import MenuSelect from '$lib/components/platform/menu-select.svelte';
	import { buttonVariants } from '$lib/components/ui/button/button.svelte';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
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
	const whereOptions = $derived([{ value: '', label: 'Every location' }, ...locations.map((l) => ({ value: l.id, label: l.name }))]);
	const roleOptions = $derived([{ value: '', label: 'Role…', disabled: true }, ...choices.map((r) => ({ value: r.name, label: r.name, hint: roleSummary(r) }))]);
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

	/** The assignment whose removal waits for its confirmation. */
	let removing = $state<RoleAssignment | null>(null);
	function removal(a: RoleAssignment) {
		const where = scopeWords(a.scope.locationId, locations);
		return isOwnLastAdmin(a)
			? {
					title: 'Remove your last admin role?',
					body: 'This is your last admin role. Removing it would immediately take away your access to people, roles, tokens, locations and the audit log, and nobody could undo it from this account. The server refuses this change; another admin has to make it. Send the request anyway?'
				}
			: { title: `Remove ${a.role}?`, body: `${principal.displayName} loses ${a.role} in ${where}.` };
	}
	async function remove(a: RoleAssignment) {
		await app.client.deleteRoleAssignment(a.id);
		onchange();
	}
	const tableColumns: TableColumn[] = [
		{ id: 'role', label: 'Role', minWidth: 220, truncate: false },
		{ id: 'where', label: 'Where', width: '160px' },
		{ id: 'from', label: 'From', width: '170px' },
		{ id: 'since', label: 'Since', width: '140px' },
		{ id: 'action', label: '', width: '150px', align: 'right' }
	];
</script>

<div class="assignments">
	<DataTable class="static framed" embedded columns={tableColumns} rows={mine} key={(a) => a.id} empty="No roles yet.">
		{#snippet row(a)}
				{@const role = roleOf(a.role)}
				{@const hint = lockedHint(a)}
				<tr class:sb-row-off={a.stale}>
					<td>
						{#if role}<a class="link" href={to(roleHref(a.role))}>{a.role}</a>{:else}<span>{a.role}</span>{/if}
						{#if role}<div class="cell-desc">{roleSummary(role)}</div>{/if}
						{#if a.stale}<div class="cell-desc warn" title="Grants nothing">Removed from code</div>{/if}
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
							{#if app.can('role.assign')}
								<DropdownMenu.Root>
									<DropdownMenu.Trigger class={buttonVariants({ variant: 'ghost', size: 'icon-sm' })} aria-label="Actions for {a.role} in {scopeWords(a.scope.locationId, locations)}" data-testid="assignment-menu"><EllipsisIcon size={14} /></DropdownMenu.Trigger>
									<DropdownMenu.Content align="end" class="w-36">
										<DropdownMenu.Item variant="destructive" onSelect={() => (removing = a)}>Remove…</DropdownMenu.Item>
									</DropdownMenu.Content>
								</DropdownMenu.Root>
							{/if}
						{:else if a.source === 'sso'}
							<a class="locked" href={to(`/admin/roles?tab=sso`)} title={hint}>See SSO mapping</a>
						{:else}
							<span class="locked" title={hint}>{a.source === 'policy' ? `Change in ${a.definedIn}` : 'Default role'}</span>
						{/if}
					</td>
				</tr>
		{/snippet}
	</DataTable>

	{#if app.can('role.assign')}
		<div class="add">
			<MenuSelect value={newLocation} options={whereOptions} onchange={(v) => (newLocation = v)} label="Where" testid="assignment-where" />
			<MenuSelect value={newRole} options={roleOptions} onchange={(v) => (newRole = v)} label="Role" testid="assignment-role" />
			<GatedButton requires="role.assign" variant="default" busy={busy === 'add'} disabled={!newRole} title="Pick a role" onclick={add}>Give role</GatedButton>
		</div>
	{/if}
	{#if error}<p class="err">{error}</p>{/if}
</div>
{#if removing}
	{@const question = removal(removing)}
	{@const target = removing}
	<ConfirmDialog open title={question.title} body={question.body} confirm="Remove" destructive onconfirm={() => remove(target)} onclose={() => (removing = null)} />
{/if}

<style>
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
	.err {
		margin-top: 8px;
		color: var(--sb-error);
		font-size: 12.5px;
	}
</style>
