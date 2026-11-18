<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import { to } from '$lib/paths';
	import RelativeTime from '$lib/components/list/relative-time.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { roleHref, scopeWords } from '$lib/platform/access';
	import { app } from '$lib/state/app.svelte';
	import type { RolesState } from './state.svelte';
	import { plural } from './utils';

	let { rs }: { rs: RolesState } = $props();
	const locations = $derived(app.bootstrap?.locations ?? []);
	const mappings = $derived(rs.sso.data ?? []);
	const unseen = $derived(mappings.filter((m) => m.membersSynced === 0));
	const tableColumns: TableColumn[] = [
		{ id: 'group', label: 'Group', minWidth: 180 },
		{ id: 'role', label: 'Gets role', width: '160px' },
		{ id: 'where', label: 'Where', width: '160px' },
		{ id: 'members', label: 'Members seen', width: '120px', align: 'right' },
		{ id: 'synced', label: 'Last synced', width: '130px' },
		{ id: 'config', label: 'Configured in', width: '24%' }
	];
</script>

<div class="px-6 pt-5 pb-8">
	{#if unseen.length}
		<p class="note">
			{plural(unseen.length, 'group')} in the config {unseen.length === 1 ? 'has' : 'have'} no members seen yet: {#each unseen as m, i (m.group)}{#if i},&nbsp;{/if}<code>{m.group}</code>{/each}.
			Check the group name matches your identity provider.
		</p>
	{/if}
	<DataTable class="static framed" embedded columns={tableColumns} rows={mappings} key={(m) => m.group + m.role} resource={rs.sso} empty="No SSO groups are mapped to roles.">
		{#snippet row(m)}
				<tr class:sb-row-off={m.membersSynced === 0}>
					<td><code>{m.group}</code></td>
					<td><a class="link" href={to(roleHref(m.role))}>{m.role}</a></td>
					<td class="muted">{scopeWords(m.scope.locationId, locations)}</td>
					<td class="num">{m.membersSynced === 0 ? 'none yet' : m.membersSynced}</td>
					<td>{#if m.lastSyncedAt}<RelativeTime value={formatAgo(m.lastSyncedAt, app.now)} exact={formatStamp(m.lastSyncedAt)} />{:else}<span class="muted">Never</span>{/if}</td>
					<td class="muted">Server config <code>{m.definedIn}</code></td>
				</tr>
		{/snippet}
	</DataTable>
</div>

<style>
	.note {
		max-width: 860px;
		margin-bottom: 14px;
		color: var(--sb-warning);
		font-size: 12.5px;
	}
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
</style>
