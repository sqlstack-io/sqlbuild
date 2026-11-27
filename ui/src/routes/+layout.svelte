<script lang="ts">
	import Button from '$lib/components/ui/button/button.svelte';
	import Loading from '$lib/components/loading.svelte';
	import './layout.css';
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import AppSidebar from '$lib/components/app-sidebar.svelte';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import { pageAvailable, pageLabel } from '$lib/nav';
	import OutagePanel from '$lib/components/shell/outage-panel.svelte';
	import { health } from '$lib/state/health.svelte';
	import { SERVER_DOCS } from '$lib/boot-failure';
	import { app } from '$lib/state/app.svelte';
	import { appPath } from '$lib/paths';
	import { DEMO, isMock, mockScenario, type SyncScenario } from '$lib/api';
	import ScenarioPicker from '$lib/components/shell/scenario-picker.svelte';
	import favicon from '$website/public/favicon.png';

	let { children } = $props();

	const path = $derived(appPath(page.url.pathname));
	const signingIn = $derived(path === '/login');
	/** Status is public: it shows without a session, and through an outage. */
	const statusPage = $derived(path === '/status');
	/** Nothing that reads the control database works: every page but Status says why and waits. */
	const blocked = $derived(health.outage !== null || app.bootWaiting);

	/** The mock's scenario picker: read from the address the app loaded with, as the mock client is. */
	let picker = $state<{ current: SyncScenario | null } | null>(null);
	onMount(() => {
		if (!signingIn) void app.start();
		const search = new URLSearchParams(window.location.search);
		if (isMock(search)) picker = { current: mockScenario(search) };
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

{#if picker && !signingIn}<ScenarioPicker current={picker.current} />{/if}

{#if signingIn}
	{@render children()}
{:else if blocked || (statusPage && !app.scope)}
	<div class="flex h-screen overflow-hidden">
		<AppSidebar {blocked} />
		<div class="flex min-w-0 flex-1 flex-col">
			{#if statusPage}
				{@render children()}
			{:else}
				<AppTopbar title={pageLabel(path)} />
				<OutagePanel page={pageLabel(path)} />
			{/if}
		</div>
	</div>
{:else if app.bootError}
	<div class="grid h-screen place-items-center p-8">
		<div class="max-w-md text-center">
			<h1 class="text-[15px] font-semibold">{app.bootError.title}</h1>
			<p class="text-muted-foreground mt-2 text-[13px]">{app.bootError.detail}</p>
			{#if app.bootError.configuration}
				<a class="link mt-4 inline-block text-[13px]" href={SERVER_DOCS} target="_blank" rel="noreferrer">Set up sign-in</a>
			{:else}
				<Button class="mt-4" onclick={() => location.reload()}>Retry</Button>
			{/if}
		</div>
	</div>
{:else if app.bootstrap && app.bootstrap.locations.length === 0}
	<div class="grid h-screen place-items-center p-8">
		<div class="max-w-md text-center">
			<h1 class="text-[15px] font-semibold">No locations to show</h1>
			<p class="text-muted-foreground mt-2 text-[13px]">Nothing deployed, or no access. Ask an administrator.</p>
		</div>
	</div>
{:else if !app.scope}
	<div class="grid h-screen place-items-center"><div class="w-60"><Loading /></div></div>
{:else}
	{#if DEMO}
		<div class="bg-primary fixed inset-x-0 top-0 z-50 h-8 truncate px-4 text-center text-[12.5px] leading-8 text-white">
			A read-only demo: DuckShop recorded on <code>sqb dev</code>. Run <code>sqb dev</code> in your own project to start runs and change settings.
		</div>
	{/if}
	<div class="flex overflow-hidden {DEMO ? 'mt-8 h-[calc(100vh-2rem)]' : 'h-screen'}">
		<AppSidebar />
		<div class="flex min-w-0 flex-1 flex-col">
			{#if pageAvailable(app.client, appPath(page.url.pathname))}
				{@render children()}
			{:else}
				<AppTopbar title="Not available" />
				<div class="text-muted-foreground grid flex-1 place-items-center p-8 text-center text-[13px]">
					<div>
						<h1 class="text-foreground text-[15px] font-semibold">Not available on this server yet</h1>
						<p class="mt-2">Not on this server version.</p>
					</div>
				</div>
			{/if}
		</div>
	</div>
{/if}
