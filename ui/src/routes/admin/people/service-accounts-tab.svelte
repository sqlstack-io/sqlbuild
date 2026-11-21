<script lang="ts">
	import BotIcon from '@lucide/svelte/icons/bot';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import type { BuiltInRole, ServiceAccount } from '$lib/api';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import { formatAgo, formatIn, formatStamp } from '$lib/format/time';
	import { errorMessage } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import AddServiceAccountPanel from './add-service-account-panel.svelte';
	import ServiceAccountPanel from './service-account-panel.svelte';
	import type { ServiceAccountsState } from './service-accounts.svelte';
	import type { ServiceState } from './types';
	import { BUILTIN_ROLES, SERVICE_STATE_LABEL, currentKey, oldKey, serviceState } from './utils';

	let { sa }: { sa: ServiceAccountsState } = $props();

	const canAdd = $derived(app.can('user.manage') && app.can('role.assign'));
	const canAssign = $derived(app.can('role.assign'));
	let query = $state('');
	const shown = $derived.by(() => {
		const q = query.trim().toLowerCase();
		return q ? sa.accounts.filter((a) => `${a.name} ${a.description ?? ''} ${a.role} ${a.keys.map((k) => k.prefix).join(' ')}`.toLowerCase().includes(q)) : sa.accounts;
	});

	const TONE: Record<ServiceState, string> = {
		active: 'var(--sb-success)',
		rotating: 'var(--sb-warning)',
		disabled: 'var(--sb-text-faint)',
		unused: 'var(--sb-text-faint)'
	};

	const columns: TableColumn[] = [
		{ id: 'name', label: 'Service account', minWidth: 200 },
		{ id: 'role', label: 'Role', width: '120px', truncate: false },
		{ id: 'key', label: 'Key', width: '220px', truncate: false },
		{ id: 'created', label: 'Created', width: '100px' },
		{ id: 'used', label: 'Last used', width: '100px' },
		{ id: 'state', label: 'State', width: '120px' }
	];

	function open(e: MouseEvent, a: ServiceAccount) {
		if ((e.target as Element).closest('a, button, input, select, label, [data-control]')) return;
		sa.pick(a.name);
	}

	let roleError = $state<{ name: string; message: string } | null>(null);
	async function setRole(a: ServiceAccount, select: HTMLSelectElement) {
		const role = select.value as BuiltInRole;
		if (role === a.role) return;
		if (role === 'admin' && !confirm(`Make ${a.name} an admin? Anyone holding its key can then manage people, roles, locations and the audit log everywhere.`)) {
			select.value = a.role;
			return;
		}
		roleError = null;
		try {
			await app.client.setServiceAccountRole(a.name, role);
		} catch (err) {
			roleError = { name: a.name, message: errorMessage(err) };
			select.value = a.role;
		}
		sa.list.refresh();
	}
</script>

<ListToolbar loading={sa.list.data === undefined} bind:query placeholder="Search service accounts, roles and key prefixes" total="{shown.length} of {sa.accounts.length} service accounts">
	{#snippet actions()}
		{#if canAdd}<Button variant="outline" size="bar" onclick={() => sa.startAdding()} data-testid="add-service-account"><PlusIcon size={14} /> Add service account</Button>{/if}
	{/snippet}
</ListToolbar>
<p class="intro">For scripts and automation, such as a deploy that republishes a project. Each signs in with an API key (<code>Authorization: Bearer sqbk_…</code>) and shows in the audit log under its name.</p>
<div class="body">
	<DataTable {columns} rows={shown} key={(a) => a.name} resource={sa.list} empty="No service accounts match.">
		{#snippet emptyRow()}
			{#if !sa.accounts.length && sa.list.data}
				<tr>
					<td colspan={columns.length}>
						<div class="empty" data-testid="service-accounts-empty">
							<BotIcon size={22} strokeWidth={1.5} />
							<div class="empty-title">No service accounts yet</div>
							<p>Add one for each script or system that calls SQLBuild, such as a deploy that republishes after each release, so its actions are audited under its own name and its key can be rotated or stopped without touching anyone's sign-in.</p>
							{#if canAdd}
								<Button size="sm" onclick={() => sa.startAdding()}><PlusIcon size={14} /> Add service account</Button>
							{:else}
								<p class="muted">Adding one needs “Manage people” (user.manage) and “Assign roles” (role.assign).</p>
							{/if}
							<p class="muted">Or from a terminal: <code>sqb admin create-service-account &lt;name&gt; --role &lt;role&gt;</code></p>
						</div>
					</td>
				</tr>
			{:else}
				<tr><td colspan={columns.length} class="muted none">No service accounts match.</td></tr>
			{/if}
		{/snippet}
		{#snippet row(a)}
			{@const status = serviceState(a)}
			{@const key = currentKey(a)}
			{@const old = oldKey(a)}
			<tr class:sb-row-off={a.disabled} class:picked={sa.selected === a.name} onclick={(e) => open(e, a)} data-testid="service-account-row">
				<td>
					<div class="cell-name">
						<BotIcon size={15} strokeWidth={1.75} />
						<div class="min-w-0">
							<Button variant="link" size="inline" class="name" onclick={() => sa.pick(a.name)}>{a.name}</Button>
							{#if a.description}<div class="cell-desc">{a.description}</div>{/if}
						</div>
					</div>
				</td>
				<td>
					{#if canAssign && !a.disabled}
						<span data-control>
							<select aria-label="Role of {a.name}" value={a.role} onchange={(e) => setRole(a, e.currentTarget)} data-testid="service-role-select">
								{#each BUILTIN_ROLES as r (r)}<option value={r}>{r}</option>{/each}
							</select>
						</span>
					{:else}
						<span>{a.role}</span>
					{/if}
					{#if roleError?.name === a.name}<div class="err" role="alert">{roleError.message}</div>{/if}
				</td>
				<td>
					{#if key}<div><code>{key.prefix}…</code>{#if key.origin === 'imported'}<span class="muted"> · imported</span>{/if}</div>{/if}
					{#if old?.expiresAt}
						<div class="cell-desc old" title="Old key, still working until {formatStamp(old.expiresAt)}">
							<code>{old.prefix}…</code> old · stops {formatIn(old.expiresAt, app.now)}
						</div>
					{/if}
				</td>
				<td><RelativeTime value={formatAgo(a.createdAt, app.now)} exact="{formatStamp(a.createdAt)} by {a.createdBy.displayName}" /></td>
				<td><RelativeTime value={a.lastUsedAt ? formatAgo(a.lastUsedAt, app.now) : 'Never'} exact={formatStamp(a.lastUsedAt)} /></td>
				<td><StatusLabel tone={TONE[status]} label={SERVICE_STATE_LABEL[status]} muted={a.disabled} /></td>
			</tr>
		{/snippet}
	</DataTable>
	{#if sa.adding}
		<AddServiceAccountPanel onadded={(result) => sa.showIssued(result, 'created')} onclose={() => sa.close()} />
	{:else if sa.selectedAccount}
		<ServiceAccountPanel account={sa.selectedAccount} {sa} />
	{/if}
</div>

<style>
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.intro {
		margin: 0;
		padding: 8px 20px;
		border-bottom: 1px solid var(--border);
		color: var(--muted-foreground);
		font-size: 12.5px;
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
	select {
		height: 26px;
		padding: 0 6px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--background);
		color: var(--foreground);
		font-size: 12.5px;
	}
	.old {
		color: var(--sb-warning) !important;
	}
	.muted {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.err {
		margin-top: 3px;
		color: var(--sb-error);
		font-size: 12px;
	}
	.none {
		padding: 24px 20px;
		text-align: center;
	}
	.empty {
		display: flex;
		max-width: 520px;
		flex-direction: column;
		align-items: center;
		gap: 8px;
		margin: 48px auto;
		color: var(--muted-foreground);
		font-size: 13px;
		text-align: center;
		white-space: normal;
	}
	.empty-title {
		color: var(--foreground);
		font-size: 14px;
		font-weight: 600;
	}
	.empty p {
		margin: 0;
	}
</style>
