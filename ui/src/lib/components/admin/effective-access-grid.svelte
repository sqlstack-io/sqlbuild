<script lang="ts">
	import type { Capability, EffectiveAccess, EffectiveAccessCell } from '$lib/api';
	import type { CheckInput } from '$lib/platform/access';
	import { locationName } from '$lib/platform/labels';
	import { PERMISSIONS_BY_RISK } from '$lib/platform/permission-catalog';
	import { app } from '$lib/state/app.svelte';
	import PermissionName from './permission-name.svelte';
	import RiskTag from './risk-tag.svelte';

	// What an account can do: actions × locations. Each mark opens the "why".
	let { access, onwhy }: { access: EffectiveAccess; onwhy: (input: CheckInput) => void } = $props();

	const locations = $derived(app.bootstrap?.locations ?? []);
	const cellOf = $derived.by(() => {
		const map = new Map<string, EffectiveAccessCell>();
		for (const c of access.cells) map.set(`${c.action}|${c.locationId}`, c);
		return (action: Capability, locationId: string) => map.get(`${action}|${locationId}`);
	});

	function why(cell: EffectiveAccessCell) {
		onwhy({ principal: access.principal, action: cell.action, locationId: cell.locationId, subject: cell.selectors[0] });
	}
</script>

<div class="grid-wrap">
	<table class="grid">
		<thead>
			<tr>
				<th class="action">Action</th>
				{#each access.locationIds as id (id)}<th>{locationName(locations, id)}</th>{/each}
			</tr>
		</thead>
		<tbody>
			{#each PERMISSIONS_BY_RISK as group (group.risk)}
				<tr class="group"><td colspan={access.locationIds.length + 1}><RiskTag risk={group.risk} /></td></tr>
				{#each group.actions as action (action)}
					<tr>
						<td class="action"><PermissionName {action} /></td>
						{#each access.locationIds as id (id)}
							{@const cell = cellOf(action, id)}
							<td>
								{#if cell && cell.coverage !== 'none'}
									<button
										class="mark {cell.coverage}"
										title="{cell.coverage === 'all' ? 'Every node' : `Only ${cell.selectors.join(', ')}`} · through {cell.roles.join(', ')}. Click to see why."
										onclick={() => why(cell)}
									>
										{#if cell.coverage === 'all'}✓{:else}✓ <code>{cell.selectors.join(', ')}</code>{/if}
									</button>
								{:else if cell}
									<button class="mark none" title="Not allowed. Click to see the nearest miss." onclick={() => why(cell)}>—</button>
								{/if}
							</td>
						{/each}
					</tr>
				{/each}
			{/each}
		</tbody>
	</table>
	<p class="legend">✓ every node · ✓ <code>selector</code> only those nodes · — not allowed. Click any mark for the reason.</p>
</div>

<style>
	.grid-wrap {
		overflow-x: auto;
	}
	.grid {
		width: 100%;
		border-collapse: collapse;
		font-size: 12.5px;
	}
	th {
		padding: 6px 10px;
		border-bottom: 1px solid var(--border);
		color: var(--muted-foreground);
		font-size: 12px;
		font-weight: 500;
		text-align: left;
		white-space: nowrap;
	}
	td {
		padding: 3px 10px;
		border-bottom: 1px solid var(--border-subtle, var(--border));
		vertical-align: middle;
	}
	.action {
		min-width: 300px;
	}
	tr.group td {
		padding-top: 12px;
		border-bottom: 0;
		color: var(--sb-text-faint);
		font-size: 11.5px;
	}
	.mark {
		padding: 1px 6px;
		border-radius: 2px;
		font-size: 12px;
		white-space: nowrap;
	}
	.mark:hover {
		background: var(--sb-hover);
	}
	.mark.all {
		color: var(--sb-success);
	}
	.mark.some {
		color: var(--sb-warning);
	}
	.mark.none {
		color: var(--sb-text-faint);
	}
	code {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.legend {
		margin-top: 8px;
		color: var(--sb-text-faint);
		font-size: 12px;
	}
</style>
