<script lang="ts">
	import { goto } from '$app/navigation';
	import UserIcon from '@lucide/svelte/icons/user';
	import ShieldQuestionIcon from '@lucide/svelte/icons/shield-question';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import ChoiceFilter from '$lib/components/list/choice-filter.svelte';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import LoadRow from '$lib/components/list/load-row.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import NoAccess from '$lib/components/platform/no-access.svelte';
	import RoleCell from '$lib/components/admin/role-cell.svelte';
	import SourceChip from '$lib/components/admin/source-chip.svelte';
	import type { AssignmentSource, RoleAssignment } from '$lib/api';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { assignmentSourceLabel, personHref } from '$lib/platform/access';
	import { accountSourceLabel } from '$lib/platform/labels';
	import { permissionLabel } from '$lib/platform/permission-catalog';
	import { app } from '$lib/state/app.svelte';
	import { createAccountTable } from '$lib/state/account-table.svelte';

	const people = createAccountTable('user');
	const d = people.directory;
	const allowed = $derived(app.can('user.manage') || app.can('role.assign'));
	const locations = $derived(app.bootstrap?.locations ?? []);
	const opensPeople = $derived(app.client.supports('effectiveAccess'));

	const sources: AssignmentSource[] = ['manual', 'sso', 'policy', 'built_in'];

	/** One chip per distinct source, e.g. one per SSO group. */
	function chips(name: string) {
		const seen = new Map<string, RoleAssignment>();
		for (const a of people.relevant(name)) seen.set(`${a.source}|${a.ssoGroup ?? ''}`, a);
		return [...seen.values()];
	}
</script>

<AppTopbar title="People">
	<Button variant="outline" size="sm" onclick={() => d.refresh()}>↻ Refresh</Button>
</AppTopbar>

{#if !allowed}
	<NoAccess what="people" requires={['user.manage', 'role.assign']} />
{:else}
	<div class="intro">
		<p>
			Everyone who has signed in, with what they can do in each location. People sign in with single sign-on, or <code>sqb login</code> on the command line.
			{#if opensPeople}Open someone to see their effective access and why.{/if}
		</p>
	</div>
	<ListToolbar bind:query={people.query} placeholder="Search people and roles" total="{people.shown.length} of {people.accounts.length} people">
		{#snippet filters()}
			<ChoiceFilter label="Role" bind:value={people.role} options={people.roleNames.map((r) => ({ value: r, label: r }))} all="Any role" />
			<ChoiceFilter label="Location" bind:value={people.locationId} options={locations.map((l) => ({ value: l.id, label: l.name }))} all="All locations" />
			<ChoiceFilter label="Source" bind:value={people.source} options={sources.map((s) => ({ value: s, label: assignmentSourceLabel[s] }))} all="Any source" />
			{#if app.client.supports('listAccessOverview')}<label class="check" title="Can change warehouse data or administer access somewhere">
				<input type="checkbox" class="sb-check" bind:checked={people.riskyOnly} />
				Has risky actions
			</label>{/if}
		{/snippet}
		{#snippet actions()}
			{#if app.client.supports('explainAccess')}<Button variant="outline" size="sm" href="/admin/check-access"><ShieldQuestionIcon size={14} /> Check access</Button>{/if}
		{/snippet}
	</ListToolbar>
	<div class="sb-list-wrap">
		<table class="sb-list">
			<thead>
				<tr>
					<th>Person</th>
					<th>{people.locationId ? `Role in ${locations.find((l) => l.id === people.locationId)?.name}` : 'Highest role'}</th>
					<th>From</th>
					<th>Last active</th>
					<th>Status</th>
				</tr>
			</thead>
			<tbody>
				{#each people.shown as a (a.name)}
					{@const risky = d.riskyOf(a.name)}
					<tr class:sb-row-off={a.disabled} class:static={!opensPeople} onclick={() => opensPeople && goto(personHref(a))}>
						<td>
							<div class="cell-name">
								<UserIcon size={15} strokeWidth={1.75} />
								<div class="min-w-0">
									{#if opensPeople}<a href={personHref(a)} onclick={(e) => e.stopPropagation()}>{a.displayName}</a>{:else}{a.displayName}{/if}
									<div class="cell-desc"><code>{a.name}</code> · {a.source === 'oidc' ? 'SSO' : a.source ? accountSourceLabel[a.source] : 'no password'}</div>
								</div>
							</div>
						</td>
						<td>
							<RoleCell assignments={d.assignmentsOf(a.name)} roles={d.allRoles} locationId={people.locationId} />
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
				{:else}
					<LoadRow colspan={5} resource={d.accounts} empty={people.accounts.length ? 'No one matches these filters.' : 'No one has signed in yet.'} />
				{/each}
			</tbody>
		</table>
	</div>
{/if}

<style>
	.intro {
		flex: 0 0 auto;
		max-width: 900px;
		padding: 14px 20px 10px;
		font-size: 12.5px;
		line-height: 1.55;
	}
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
	tr.static {
		cursor: default;
	}
</style>
