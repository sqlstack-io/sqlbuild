<script lang="ts">
	import PageBody from '$lib/components/page-body.svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import NoAccess from '$lib/components/platform/no-access.svelte';
	import PageTabs from '$lib/components/platform/page-tabs.svelte';
	import { app } from '$lib/state/app.svelte';
	import RolesTab from './roles-tab.svelte';
	import SsoTab from './sso-tab.svelte';
	import { createRolesState } from './state.svelte';
	import { parseTab, type RolesTab as Tab } from './utils';

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
	<RefreshButton onclick={() => rs.refresh()} />
</AppTopbar>

{#if !app.can('role.assign')}
	<NoAccess what="roles" requires="role.assign" />
{:else}
	<PageTabs
		bind:value={() => tab, select}
		tabs={[
			{ value: 'roles', label: 'Roles', count: rs.roles.data?.length },
			...(app.client.supports('listSsoMappings') ? [{ value: 'sso' as const, label: 'SSO mappings', count: rs.sso.data?.length }] : [])
		]}
	/>
	<PageBody padding="0">
		{#if tab === 'roles' || !app.client.supports('listSsoMappings')}
			<RolesTab {rs} />
		{:else}
			<SsoTab {rs} />
		{/if}
	</PageBody>
{/if}

