<script lang="ts">
	import { goto } from '$app/navigation';
	import LoadRow from '$lib/components/list/load-row.svelte';
	import RoleSourceBadge from '$lib/components/admin/role-source-badge.svelte';
	import { roleHref, roleSummary } from '$lib/platform/access';
	import { app } from '$lib/state/app.svelte';
	import type { RolesState } from './state.svelte';
	import { plural } from './utils';

	let { rs }: { rs: RolesState } = $props();
</script>

<div class="px-6 pt-5 pb-8">
	<p class="lede">
		Roles in <b>{app.location?.name}</b>. Each custom role is a built-in role plus additions on chosen nodes. Roles are defined in code (the project's policy file or the server
		config) and are read-only here.
	</p>
	<table class="sb-list framed">
		<thead>
			<tr>
				<th>Role</th>
				<th>Defined in</th>
				<th>What it gives</th>
				<th class="num">Who has it</th>
			</tr>
		</thead>
		<tbody>
			{#each rs.roles.data ?? [] as r (r.name)}
				<tr onclick={() => goto(roleHref(r.name))}>
					<td>
						<a class="link" href={roleHref(r.name)} onclick={(e) => e.stopPropagation()}><strong>{r.name}</strong></a>
						<div class="cell-desc">{r.description}</div>
					</td>
					<td><RoleSourceBadge role={r} /></td>
					<td>
						{roleSummary(r)}
						{#if r.recentlyMatched}<div class="cell-desc growth" title="Selectors match more nodes than a week ago">+{plural(r.recentlyMatched, 'node')} matched this week</div>{/if}
					</td>
					<td class="num">{r.holders}</td>
				</tr>
			{:else}
				<LoadRow colspan={4} resource={rs.roles} empty="No roles." />
			{/each}
		</tbody>
	</table>
</div>

<style>
	.lede {
		max-width: 860px;
		margin-bottom: 18px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.lede b {
		color: var(--foreground);
		font-weight: 500;
	}
	.framed {
		border-top: 1px solid var(--border);
	}
	.growth {
		color: var(--sb-warning) !important;
	}
</style>
