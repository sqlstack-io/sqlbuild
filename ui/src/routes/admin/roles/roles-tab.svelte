<script lang="ts">
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import { openRow, to } from '$lib/paths';
	import RoleSourceBadge from '$lib/components/admin/role-source-badge.svelte';
	import { roleHref, roleSummary } from '$lib/platform/access';
	import { app } from '$lib/state/app.svelte';
	import type { RolesState } from './state.svelte';
	import { plural } from './utils';

	let { rs }: { rs: RolesState } = $props();
	const tableColumns: TableColumn[] = [
		{ id: 'role', label: 'Role', minWidth: 220 },
		{ id: 'source', label: 'Defined in', width: '180px' },
		{ id: 'gives', label: 'What it gives', width: '34%', truncate: false },
		{ id: 'holders', label: 'Who has it', width: '110px', align: 'right' }
	];
</script>

<div class="px-6 pt-5 pb-8">
	<DataTable class="framed" embedded columns={tableColumns} rows={rs.roles.data ?? []} key={(r) => r.name} resource={rs.roles} empty="No roles.">
		{#snippet row(r)}
				<tr onclick={(e) => openRow(e, roleHref(r.name))}>
					<td>
						<a class="link" href={to(roleHref(r.name))}><strong>{r.name}</strong></a>
						<div class="cell-desc">{r.description}</div>
					</td>
					<td><RoleSourceBadge role={r} /></td>
					<td>
						{roleSummary(r)}
						{#if r.recentlyMatched}<div class="cell-desc growth" title="Selectors match more nodes than a week ago">+{plural(r.recentlyMatched, 'node')} matched this week</div>{/if}
					</td>
					<td class="num">{r.holders}</td>
				</tr>
		{/snippet}
	</DataTable>
</div>

<style>
	.growth {
		color: var(--sb-warning) !important;
	}
</style>
