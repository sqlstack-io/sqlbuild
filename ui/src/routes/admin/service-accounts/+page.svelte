<script lang="ts">
	import { goto } from '$app/navigation';
	import BotIcon from '@lucide/svelte/icons/bot';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import ChoiceFilter from '$lib/components/list/choice-filter.svelte';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import LoadRow from '$lib/components/list/load-row.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import GatedButton from '$lib/components/platform/gated-button.svelte';
	import NoAccess from '$lib/components/platform/no-access.svelte';
	import RoleCell from '$lib/components/admin/role-cell.svelte';
	import SourceChip from '$lib/components/admin/source-chip.svelte';
	import type { AssignmentSource, RoleAssignment } from '$lib/api';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { assignmentSourceLabel, personHref } from '$lib/platform/access';
	import { errorMessage } from '$lib/platform/labels';
	import { permissionLabel } from '$lib/platform/permission-catalog';
	import { createAccountTable } from '$lib/state/account-table.svelte';
	import { app } from '$lib/state/app.svelte';

	const table = createAccountTable('service_account');
	const d = table.directory;
	const allowed = $derived(app.can('user.manage') || app.can('token.manage'));
	const locations = $derived(app.bootstrap?.locations ?? []);
	const sources: AssignmentSource[] = ['manual', 'policy'];

	function chips(name: string) {
		const seen = new Map<string, RoleAssignment>();
		for (const a of table.relevant(name)) seen.set(`${a.source}|${a.ssoGroup ?? ''}`, a);
		return [...seen.values()];
	}

	let creating = $state(false);
	let name = $state('');
	let description = $state('');
	let busy = $state(false);
	let error = $state<string | null>(null);

	async function create() {
		busy = true;
		error = null;
		try {
			const account = await app.client.createServiceAccount({ name: name.trim(), description: description.trim() });
			await goto(personHref(account));
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}
</script>

<AppTopbar title="Service accounts">
	<Button variant="outline" size="sm" onclick={() => d.refresh()}>↻ Refresh</Button>
</AppTopbar>

{#if !allowed}
	<NoAccess what="service accounts" requires={['user.manage', 'token.manage']} />
{:else}
	<div class="intro">
		<p>
			Accounts for scripts and CI. They sign in only with API tokens, whose secret is shown once. They get roles like people do. People don't need tokens: they use <code>sqb login</code>.
			Tokens for code locations are on each <a href="/locations">location's page</a>.
		</p>
	</div>
	<ListToolbar bind:query={table.query} placeholder="Search service accounts" total="{table.shown.length} of {table.accounts.length} accounts">
		{#snippet filters()}
			<ChoiceFilter label="Role" bind:value={table.role} options={table.roleNames.map((r) => ({ value: r, label: r }))} all="Any role" />
			<ChoiceFilter label="Location" bind:value={table.locationId} options={locations.map((l) => ({ value: l.id, label: l.name }))} all="All locations" />
			<ChoiceFilter label="Source" bind:value={table.source} options={sources.map((s) => ({ value: s, label: assignmentSourceLabel[s] }))} all="Any source" />
			<label class="check" title="Can change warehouse data or administer access somewhere">
				<input type="checkbox" class="sb-check" bind:checked={table.riskyOnly} />
				Has risky actions
			</label>
		{/snippet}
		{#snippet actions()}
			<GatedButton requires="user.manage" onclick={() => (creating = !creating)}>New service account</GatedButton>
		{/snippet}
	</ListToolbar>
	{#if creating}
		<div class="create">
			<input class="field" placeholder="Name, e.g. svc-exports" bind:value={name} aria-label="Service account name" />
			<input class="field wide" placeholder="What it's for" bind:value={description} aria-label="What it's for" />
			<GatedButton requires="user.manage" variant="default" {busy} disabled={!name.trim()} title="Give it a name" onclick={create}>Create</GatedButton>
			<Button variant="ghost" size="sm" onclick={() => (creating = false)}>Cancel</Button>
			{#if error}<span class="err">{error}</span>{/if}
		</div>
	{/if}
	<div class="sb-list-wrap">
		<table class="sb-list">
			<thead>
				<tr>
					<th>Account</th>
					<th>{table.locationId ? `Role in ${locations.find((l) => l.id === table.locationId)?.name}` : 'Highest role'}</th>
					<th>From</th>
					<th>Owner</th>
					<th class="num">Tokens</th>
					<th>Last used</th>
					<th>Status</th>
				</tr>
			</thead>
			<tbody>
				{#each table.shown as a (a.name)}
					{@const risky = d.riskyOf(a.name)}
					{@const active = d.tokensOf(a.name).filter((t) => t.state === 'active').length}
					<tr class:sb-row-off={a.disabled} onclick={() => goto(personHref(a))}>
						<td>
							<div class="cell-name">
								<BotIcon size={15} strokeWidth={1.75} />
								<div class="min-w-0">
									<a href={personHref(a)} onclick={(e) => e.stopPropagation()}><code>{a.name}</code></a>
									<div class="cell-desc">{a.description ?? '—'}</div>
								</div>
							</div>
						</td>
						<td>
							<RoleCell assignments={d.assignmentsOf(a.name)} roles={d.allRoles} locationId={table.locationId} />
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
						<td class="muted">{a.owner ?? '—'}</td>
						<td class="num">{app.can('token.manage') ? active : '—'}</td>
						<td><RelativeTime value={a.lastSeenAt ? formatAgo(a.lastSeenAt, app.now) : 'Never'} exact={formatStamp(a.lastSeenAt)} /></td>
						<td><StatusLabel tone={a.disabled ? 'var(--sb-text-faint)' : 'var(--sb-success)'} label={a.disabled ? 'Disabled' : 'Active'} muted={a.disabled} /></td>
					</tr>
				{:else}
					<LoadRow colspan={7} resource={d.accounts} empty={table.accounts.length ? 'No service accounts match these filters.' : 'No service accounts yet.'} />
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
	a {
		color: var(--primary);
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
	.create {
		display: flex;
		flex: 0 0 auto;
		align-items: center;
		gap: 8px;
		padding: 10px 20px;
		border-bottom: 1px solid var(--border);
	}
	.field {
		width: 240px;
		height: 30px;
		padding: 0 9px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		color: var(--foreground);
		font-size: 12.5px;
	}
	.field.wide {
		width: 340px;
	}
	.err {
		color: var(--sb-error);
		font-size: 12.5px;
	}
</style>
