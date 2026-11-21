<script lang="ts">
	import BotIcon from '@lucide/svelte/icons/bot';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import type { ServiceAccount } from '$lib/api';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import { formatAgo, formatIn, formatStamp } from '$lib/format/time';
	import { app } from '$lib/state/app.svelte';
	import AddServiceAccountDialog from './add-service-account-dialog.svelte';
	import ServiceAccountPanel from './service-account-panel.svelte';
	import type { ServiceAccountsState } from './service-accounts.svelte';
	import type { ServiceState } from './types';
	import { SERVICE_STATE_LABEL, currentKey, oldKey, serviceState } from './utils';

	let { sa }: { sa: ServiceAccountsState } = $props();

	const canAdd = $derived(app.can('user.manage') && app.can('role.assign'));
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

	// State comes early, so it stays in view when an open panel leaves the list too narrow for every column.
	const columns: TableColumn[] = [
		{ id: 'name', label: 'Service account', minWidth: 220 },
		{ id: 'state', label: 'State', width: '116px' },
		{ id: 'role', label: 'Role', width: '96px' },
		{ id: 'key', label: 'Key', width: '170px', truncate: false },
		{ id: 'used', label: 'Last used', width: '100px' },
		{ id: 'created', label: 'Created', width: '100px' }
	];

	/** A click on a row opens its panel, and on the open row closes it. */
	function onrow(e: MouseEvent, a: ServiceAccount) {
		if ((e.target as Element).closest('a, button, input, select, label, [data-control]')) return;
		sa.toggle(a.name);
	}
</script>

<ListToolbar loading={sa.list.data === undefined} bind:query placeholder="Search service accounts" total="{shown.length} of {sa.accounts.length} service accounts">
	{#snippet actions()}
		{#if canAdd}<Button variant="outline" size="bar" onclick={() => sa.setAdding(true)} data-testid="add-service-account"><PlusIcon size={14} /> Add service account</Button>{/if}
	{/snippet}
</ListToolbar>
<div class="body">
	<DataTable {columns} rows={shown} key={(a) => a.name} resource={sa.list} empty="No service accounts match.">
		{#snippet emptyRow()}
			{#if !sa.accounts.length && sa.list.data}
				<tr>
					<td colspan={columns.length}>
						<div class="empty" data-testid="service-accounts-empty">
							<BotIcon size={22} strokeWidth={1.5} />
							<div class="empty-title">No service accounts yet</div>
							<p>A service account lets a script or system call SQLBuild with an API key, such as a deploy that republishes after each release. Its actions appear in the audit log under its own name.</p>
							{#if canAdd}
								<Button size="sm" onclick={() => sa.setAdding(true)}><PlusIcon size={14} /> Add service account</Button>
							{/if}
							<p class="muted">From a terminal: <code>sqb admin create-service-account &lt;name&gt; --role &lt;role&gt;</code></p>
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
			<tr class:sb-row-off={a.disabled} class:picked={sa.selected === a.name} onclick={(e) => onrow(e, a)} data-testid="service-account-row">
				<td>
					<div class="cell-name">
						<BotIcon size={15} strokeWidth={1.75} />
						<div class="min-w-0">
							<Button variant="link" size="inline" class="name" onclick={() => sa.toggle(a.name)}>{a.name}</Button>
							{#if a.description}<div class="cell-desc">{a.description}</div>{/if}
						</div>
					</div>
				</td>
				<td><StatusLabel tone={TONE[status]} label={SERVICE_STATE_LABEL[status]} muted={a.disabled} /></td>
				<td>{a.role}</td>
				<td>
					{#if key}<code>{key.prefix}…</code>{:else}<span class="muted">No key</span>{/if}
					{#if old?.expiresAt}
						<div class="old" title="The old key works until {formatStamp(old.expiresAt)}"><code>{old.prefix}…</code> stops {formatIn(old.expiresAt, app.now)}</div>
					{/if}
				</td>
				<td><RelativeTime value={a.lastUsedAt ? formatAgo(a.lastUsedAt, app.now) : 'Never'} exact={formatStamp(a.lastUsedAt)} /></td>
				<td><RelativeTime value={formatAgo(a.createdAt, app.now)} exact={formatStamp(a.createdAt)} /></td>
			</tr>
		{/snippet}
	</DataTable>
	{#if sa.selectedAccount}
		<ServiceAccountPanel account={sa.selectedAccount} {sa} />
	{/if}
</div>
<AddServiceAccountDialog
	open={sa.adding}
	onadded={(name) => {
		sa.list.refresh();
		sa.open(name);
	}}
	onclose={() => sa.setAdding(false)}
/>

<style>
	code {
		font-family: var(--font-mono);
		font-size: 12px;
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
	code {
		white-space: nowrap;
	}
	.old {
		color: var(--sb-warning);
		font-size: 11.5px;
		white-space: nowrap;
	}
	.old code {
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.muted {
		color: var(--muted-foreground);
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
