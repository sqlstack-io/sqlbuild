<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import { openRow, to } from '$lib/paths';
	import UserIcon from '@lucide/svelte/icons/user';
	import ShieldQuestionIcon from '@lucide/svelte/icons/shield-question';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import ChoiceFilter from '$lib/components/list/choice-filter.svelte';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import NoAccess from '$lib/components/platform/no-access.svelte';
	import RoleCell from '$lib/components/admin/role-cell.svelte';
	import SourceChip from '$lib/components/admin/source-chip.svelte';
	import type { Account, AssignmentSource, RoleAssignment } from '$lib/api';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { assignmentSourceLabel, personHref } from '$lib/platform/access';
	import { accountSourceLabel } from '$lib/platform/labels';
	import { permissionLabel } from '$lib/platform/permission-catalog';
	import { app } from '$lib/state/app.svelte';
	import { createAccountTable } from '$lib/state/account-table.svelte';
	import UserPlusIcon from '@lucide/svelte/icons/user-plus';
	import AddPersonPanel from './add-person-panel.svelte';
	import PersonPanel from './person-panel.svelte';
	import RoleSelect from './role-select.svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import PageTabs from '$lib/components/platform/page-tabs.svelte';
	import ServiceAccountsTab from './service-accounts-tab.svelte';
	import { createServiceAccountsState } from './service-accounts.svelte';
	import type { PeopleTab } from './types';
	import { defaultRoleOf, isLastAdmin, parsePeopleTab } from './utils';

	const people = createAccountTable('user');
	// Service accounts sit in a tab beside People; servers without them show People alone.
	const hasServiceAccounts = $derived(app.client.supports('listServiceAccounts'));
	const tab = $derived(hasServiceAccounts ? parsePeopleTab(page.url.searchParams.get('tab')) : 'people');
	const sa = createServiceAccountsState(() => page.url.searchParams.get('open'));
	function selectTab(value: PeopleTab) {
		const url = new URL(page.url);
		url.searchParams.delete('open');
		if (value === 'people') url.searchParams.delete('tab');
		else url.searchParams.set('tab', value);
		void goto(url, { replaceState: true, noScroll: true, keepFocus: true });
	}
	const d = people.directory;
	const allowed = $derived(app.can('user.manage') || app.can('role.assign'));
	const locations = $derived(app.bootstrap?.locations ?? []);
	const canAssign = $derived(app.can('role.assign'));
	const defaultRole = $derived(defaultRoleOf(d.assignments.data ?? []));
	// Behind SSO or a trusted proxy, people arrive by signing in; with password sign-in they are added here.
	const arriveBySigningIn = $derived(app.bootstrap?.authMode !== 'password');
	const canAdd = $derived(!arriveBySigningIn && app.can('user.manage') && app.client.supports('createPasswordAccount'));
	let adding = $state(false);
	function added(a: Account) {
		d.refresh();
		pick(a.name);
	}

	/** The person whose panel is open beside the list. */
	let selected = $state<string | null>(null);
	const selectedAccount = $derived(selected ? (people.accounts.find((a) => a.name === selected) ?? null) : null);
	/** A plain click opens the panel; a modifier click opens their full page in a new tab, as other lists do. */
	function open(e: MouseEvent, a: Account) {
		if ((e.target as Element).closest('a, button, input, select, label, [data-control]')) return;
		if (e.metaKey || e.ctrlKey || e.button === 1) openRow(e, personHref(a));
		else pick(a.name);
	}
	/** Open a person's panel in place of whatever panel is open. */
	function pick(name: string) {
		adding = false;
		selected = name;
	}

	const sources: AssignmentSource[] = ['manual', 'sso', 'policy', 'built_in'];

	/** One chip per distinct source, e.g. one per SSO group. */
	function chips(name: string) {
		const seen = new Map<string, RoleAssignment>();
		for (const a of people.relevant(name)) seen.set(`${a.source}|${a.ssoGroup ?? ''}`, a);
		return [...seen.values()];
	}
	const tableColumns = $derived<TableColumn[]>([
		{ id: 'person', label: 'Person', minWidth: 220 },
		{ id: 'role', label: people.locationId ? `Role in ${locations.find((l) => l.id === people.locationId)?.name}` : 'Role', width: '26%', truncate: false },
		{ id: 'from', label: 'From', width: '22%', truncate: false },
		{ id: 'active', label: 'Last active', width: '130px' },
		{ id: 'status', label: 'Status', width: '110px' }
	]);
</script>

<AppTopbar title="People">
	<RefreshButton onclick={() => (tab === 'service-accounts' ? sa.list.refresh() : d.refresh())} />
</AppTopbar>

{#if !allowed}
	<NoAccess what={tab === 'service-accounts' ? 'service accounts' : 'people'} requires={['user.manage', 'role.assign']} />
{:else}
	{#if hasServiceAccounts}
		<PageTabs
			bind:value={() => tab, selectTab}
			tabs={[
				{ value: 'people', label: 'People', count: d.accounts.data ? people.accounts.length : undefined },
				{ value: 'service-accounts', label: 'Service accounts', count: sa.list.data?.length, title: 'Scripts and automation that call SQLBuild with an API key' }
			]}
		/>
	{/if}
	{#if tab === 'service-accounts'}
		<ServiceAccountsTab {sa} />
	{:else}
	<ListToolbar loading={d.accounts.data === undefined} bind:query={people.query} placeholder="Search people and roles" total="{people.shown.length} of {people.accounts.length} people">
		{#snippet filters()}
			<ChoiceFilter multiple label="Role" bind:values={people.role} options={people.roleNames.map((r) => ({ value: r, label: r }))} />
			<ChoiceFilter label="Location" bind:value={people.locationId} options={locations.map((l) => ({ value: l.id, label: l.name }))} all="All locations" />
			<ChoiceFilter multiple label="Source" bind:values={people.source} options={sources.map((s) => ({ value: s, label: assignmentSourceLabel[s] }))} />
			{#if app.client.supports('listAccessOverview')}<label class="check" title="Can change warehouse data or administer access somewhere">
				<input type="checkbox" class="sb-check" bind:checked={people.riskyOnly} />
				Has risky actions
			</label>{/if}
		{/snippet}
		{#snippet actions()}
			{#if canAdd}<Button variant="outline" size="bar" onclick={() => ((selected = null), (adding = true))} data-testid="add-person"><UserPlusIcon size={14} /> Add person</Button>{/if}
			{#if app.client.supports('explainAccess')}<Button variant="outline" size="bar" href={to(`/admin/check-access`)}><ShieldQuestionIcon size={14} /> Check access</Button>{/if}
		{/snippet}
	</ListToolbar>
	{#if arriveBySigningIn}<p class="arrive" data-testid="people-arrive">People appear here the first time they sign in. Pick a role to change it.</p>{/if}
	<div class="body">
	<DataTable columns={tableColumns} rows={people.shown} key={(a) => a.name} resource={d.accounts} empty={people.accounts.length ? 'No one matches these filters.' : 'No one has signed in yet.'}>
		{#snippet row(a)}
					{@const risky = d.riskyOf(a.name)}
					<tr class:sb-row-off={a.disabled} class:picked={selected === a.name} onclick={(e) => open(e, a)}>
						<td>
							<div class="cell-name">
								<UserIcon size={15} strokeWidth={1.75} />
								<div class="min-w-0">
									<Button variant="link" size="inline" class="name" onclick={() => pick(a.name)}>{a.displayName}</Button>
									<div class="cell-desc"><code>{a.name}</code> · {a.source === 'oidc' ? 'SSO' : a.source ? accountSourceLabel[a.source] : 'no password'}</div>
								</div>
							</div>
						</td>
						<td>
							{#if people.locationId}
								<RoleCell assignments={d.assignmentsOf(a.name)} roles={d.allRoles} locationId={people.locationId} />
							{:else}
								<RoleSelect account={a} assignments={d.assignmentsOf(a.name)} {defaultRole} lastAdmin={isLastAdmin(a.name, d.assignments.data ?? [], people.accounts)} {canAssign} onchange={() => d.refresh()} />
							{/if}
							{#if risky.length}
								<div class="cell-desc risky" title={risky.map((r) => `${permissionLabel(r)} (${r})`).join('\n')}>
									<TriangleAlertIcon size={11} /> Can {permissionLabel(risky[0]).toLowerCase()}{risky.length > 1 ? ` and ${risky.length - 1} more risky actions` : ''}
								</div>
							{/if}
						</td>
						<td>
							<div class="chips">
								{#each chips(a.name) as c (`${c.source}|${c.ssoGroup}`)}<SourceChip assignment={c} />{/each}
							</div>
						</td>
						<td><RelativeTime value={a.lastSeenAt ? formatAgo(a.lastSeenAt, app.now) : 'Never'} exact={formatStamp(a.lastSeenAt)} /></td>
						<td>
							<StatusLabel tone={a.disabled ? 'var(--sb-text-faint)' : 'var(--sb-success)'} label={a.disabled ? 'Disabled' : 'Active'} muted={a.disabled} />
							{#if a.name === app.user?.name}<div class="cell-desc">you</div>{/if}
						</td>
					</tr>
		{/snippet}
	</DataTable>
	{#if adding}
		<AddPersonPanel {defaultRole} onadded={added} onclose={() => (adding = false)} />
	{:else if selectedAccount}
		<PersonPanel account={selectedAccount} assignments={d.assignmentsOf(selectedAccount.name)} rolesByLocation={d.rolesByLocation} onchange={() => d.refresh()} onclose={() => (selected = null)} />
	{/if}
	</div>
{/if}
{/if}

<style>
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.check {
		display: inline-flex;
		align-items: center;
		gap: 7px;
		margin-left: 6px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}
	.risky {
		display: flex;
		align-items: center;
		gap: 4px;
		color: var(--sb-warning) !important;
	}
	.body {
		display: flex;
		min-height: 0;
		flex: 1;
	}
	.body > :global(:first-child) {
		min-width: 0;
		flex: 1;
	}
	tr.picked {
		background: var(--sb-row-selected, color-mix(in srgb, var(--primary) 8%, transparent));
	}
	.arrive {
		margin: 0;
		padding: 8px 20px;
		border-bottom: 1px solid var(--border);
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
</style>
