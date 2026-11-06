<script lang="ts">
	import './layout.css';
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import AppSidebar from '$lib/components/app-sidebar.svelte';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import { pageAvailable } from '$lib/nav';
	import { SERVER_DOCS } from '$lib/boot-failure';
	import { app } from '$lib/state/app.svelte';

	let { children } = $props();

	const signingIn = $derived(page.url.pathname === '/login');

	onMount(() => {
		if (!signingIn) void app.start();
	});
</script>

<svelte:head>
	<link rel="icon" href="/favicon.png" />
</svelte:head>

{#if signingIn}
	{@render children()}
{:else if app.bootError}
	<div class="grid h-screen place-items-center p-8">
		<div class="max-w-md text-center">
			<h1 class="text-[15px] font-semibold">{app.bootError.title}</h1>
			<p class="text-muted-foreground mt-2 text-[13px]">{app.bootError.detail}</p>
			{#if app.bootError.configuration}
				<a class="link mt-4 inline-block text-[13px]" href={SERVER_DOCS} target="_blank" rel="noreferrer">Set up sign-in</a>
			{:else}
				<button class="mt-4 rounded-md border border-border px-3 py-1.5 text-[13px]" onclick={() => location.reload()}>Retry</button>
			{/if}
		</div>
	</div>
{:else if app.bootstrap && app.bootstrap.locations.length === 0}
	<div class="grid h-screen place-items-center p-8">
		<div class="max-w-md text-center">
			<h1 class="text-[15px] font-semibold">No locations to show</h1>
			<p class="text-muted-foreground mt-2 text-[13px]">Either nothing is deployed yet, or you have no role on a location. Ask an administrator for access, then reload this page.</p>
		</div>
	</div>
{:else if !app.scope}
	<div class="text-muted-foreground grid h-screen place-items-center text-[13px]">Loading…</div>
{:else}
	<div class="flex h-screen overflow-hidden">
		<AppSidebar />
		<div class="flex min-w-0 flex-1 flex-col">
			{#if pageAvailable(app.client, page.url.pathname)}
				{@render children()}
			{:else}
				<AppTopbar title="Not available" />
				<div class="text-muted-foreground grid flex-1 place-items-center p-8 text-center text-[13px]">
					<div>
						<h1 class="text-foreground text-[15px] font-semibold">Not available on this server yet</h1>
						<p class="mt-2">This server version doesn't provide this page. Choose another page from the menu.</p>
					</div>
				</div>
			{/if}
		</div>
	</div>
{/if}
