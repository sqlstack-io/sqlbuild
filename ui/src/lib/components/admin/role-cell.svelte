<script lang="ts">
	import { to } from '$lib/paths';
	import * as Popover from '$lib/components/ui/popover';
	import type { RoleAssignment, RoleDefinition } from '$lib/api';
	import { assignmentsAt, rankAssignments, roleHref, scopeWords } from '$lib/platform/access';
	import { app } from '$lib/state/app.svelte';
	import SourceChip from './source-chip.svelte';

	// Role per location: with a location chosen, the roles that apply there;
	// otherwise the highest role anywhere, with the rest in a popover.
	let { assignments, roles, locationId }: { assignments: RoleAssignment[]; roles: RoleDefinition[]; locationId: string } = $props();

	const locations = $derived(app.bootstrap?.locations ?? []);
	const shown = $derived(rankAssignments(locationId ? assignmentsAt(assignments, locationId) : assignments, roles));
	const top = $derived(shown[0]);
	const rest = $derived(shown.length - 1);
	let open = $state(false);
</script>

{#if top}
	<span class="cell">
		<span class:stale={top.stale}>{top.role}</span>
		{#if !locationId}<span class="where">{top.scope.locationId === null ? 'everywhere' : `in ${scopeWords(top.scope.locationId, locations)}`}</span>{/if}
		{#if rest > 0}
			<Popover.Root bind:open>
				<Popover.Trigger class="more" onclick={(e: MouseEvent) => e.stopPropagation()}>+{rest} more</Popover.Trigger>
				<Popover.Content class="w-[340px] p-0" align="start">
					<div class="border-b border-border px-3 py-2 text-[12px] font-semibold">
						{locationId ? `Roles in ${scopeWords(locationId, locations)}` : 'All roles'}
					</div>
					<ul class="list">
						{#each shown as a (a.id)}
							<li class:stale={a.stale}>
								<a href={to(roleHref(a.role))}>{a.role}</a>
								<span class="where">{scopeWords(a.scope.locationId, locations)}</span>
								<SourceChip assignment={a} />
								{#if a.stale}<span class="where">gives nothing</span>{/if}
							</li>
						{/each}
					</ul>
				</Popover.Content>
			</Popover.Root>
		{/if}
	</span>
{:else}
	<span class="none">No role here</span>
{/if}

<style>
	.cell {
		display: inline-flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 2px 7px;
	}
	.where {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.stale {
		color: var(--sb-text-faint);
		text-decoration: line-through;
	}
	.none {
		color: var(--sb-text-faint);
	}
	.cell :global(.more) {
		color: var(--primary);
		font-size: 12px;
	}
	.list {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 8px 12px 10px;
		font-size: 12.5px;
	}
	.list li {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
	}
	.list a {
		color: var(--primary);
	}
	li.stale a {
		text-decoration: line-through;
	}
</style>
