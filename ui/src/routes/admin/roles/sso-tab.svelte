<script lang="ts">
	import LoadRow from '$lib/components/list/load-row.svelte';
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
</script>

<div class="px-6 pt-5 pb-8">
	{#if unseen.length}
		<p class="note">
			{plural(unseen.length, 'group')} in the config {unseen.length === 1 ? 'has' : 'have'} no members seen yet: {#each unseen as m, i (m.group)}{#if i},&nbsp;{/if}<code>{m.group}</code>{/each}.
			Check the group name matches your identity provider.
		</p>
	{/if}
	<table class="sb-list static framed">
		<thead>
			<tr>
				<th>Group</th>
				<th>Gets role</th>
				<th>Where</th>
				<th class="num">Members seen</th>
				<th>Last synced</th>
				<th>Configured in</th>
			</tr>
		</thead>
		<tbody>
			{#each mappings as m (m.group + m.role)}
				<tr class:sb-row-off={m.membersSynced === 0}>
					<td><code>{m.group}</code></td>
					<td><a class="link" href={roleHref(m.role)}>{m.role}</a></td>
					<td class="muted">{scopeWords(m.scope.locationId, locations)}</td>
					<td class="num">{m.membersSynced === 0 ? 'none yet' : m.membersSynced}</td>
					<td>{#if m.lastSyncedAt}<RelativeTime value={formatAgo(m.lastSyncedAt, app.now)} exact={formatStamp(m.lastSyncedAt)} />{:else}<span class="muted">Never</span>{/if}</td>
					<td class="muted">Server config <code>{m.definedIn}</code></td>
				</tr>
			{:else}
				<LoadRow colspan={6} resource={rs.sso} empty="No SSO groups are mapped to roles." />
			{/each}
		</tbody>
	</table>
</div>

<style>
	:global(.sb-list.static tbody tr) {
		cursor: default;
	}
	.note {
		max-width: 860px;
		margin-bottom: 14px;
		color: var(--sb-warning);
		font-size: 12.5px;
	}
	.framed {
		border-top: 1px solid var(--border);
	}
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
</style>
