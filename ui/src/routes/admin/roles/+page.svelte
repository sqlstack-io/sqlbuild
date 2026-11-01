<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import ClockIcon from '@lucide/svelte/icons/clock-3';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import NoAccess from '$lib/components/platform/no-access.svelte';
	import PageTabs from '$lib/components/platform/page-tabs.svelte';
	import { app } from '$lib/state/app.svelte';
	import ApprovalsTab from './approvals-tab.svelte';
	import RolesTab from './roles-tab.svelte';
	import SsoTab from './sso-tab.svelte';
	import { createRolesState } from './state.svelte';
	import { parseTab, plural, type RolesTab as Tab } from './utils';

	const rs = createRolesState();
	const tab = $derived(parseTab(page.url.searchParams.get('tab')));

	function select(value: Tab) {
		const url = new URL(page.url);
		if (value === 'roles') url.searchParams.delete('tab');
		else url.searchParams.set('tab', value);
		void goto(url, { replaceState: true, noScroll: true, keepFocus: true });
	}
</script>

<AppTopbar title="Roles">
	<Button variant="outline" size="sm" onclick={() => rs.refresh()}>↻ Refresh</Button>
</AppTopbar>

{#if !app.can('role.assign')}
	<NoAccess what="roles" requires="role.assign" />
{:else}
	<PageTabs
		bind:value={() => tab, select}
		tabs={[
			{ value: 'roles', label: 'Roles', count: rs.roles.data?.length },
			{ value: 'sso', label: 'SSO mappings', count: rs.sso.data?.length },
			{ value: 'approvals', label: 'Approvals', count: rs.pending.length }
		]}
	/>
	{#if rs.pending.length && tab !== 'approvals'}
		<button class="banner" onclick={() => select('approvals')}>
			<ClockIcon size={14} />
			<span><b>{plural(rs.pending.length, 'role change')} waiting for approval</b> ({[...new Set(rs.pending.map((a) => a.role))].join(', ')}). Until approved, the roles keep their current actions.</span>
			<span class="go">Review →</span>
		</button>
	{/if}
	<div class="min-h-0 flex-1 overflow-auto">
		{#if tab === 'roles'}
			<RolesTab {rs} />
		{:else if tab === 'sso'}
			<SsoTab {rs} />
		{:else}
			<ApprovalsTab {rs} />
		{/if}
	</div>
{/if}

<style>
	.banner {
		display: flex;
		flex: 0 0 auto;
		align-items: center;
		gap: 8px;
		padding: 9px 20px;
		border-bottom: 1px solid color-mix(in srgb, var(--sb-warning) 35%, var(--border));
		background: color-mix(in srgb, var(--sb-warning) 8%, var(--background));
		font-size: 12.5px;
		text-align: left;
	}
	.banner :global(svg) {
		flex: 0 0 auto;
		color: var(--sb-warning);
	}
	.banner b {
		font-weight: 600;
	}
	.go {
		margin-left: auto;
		color: var(--primary);
		white-space: nowrap;
	}
</style>
