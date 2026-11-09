<script lang="ts">
	import './layout.css';
	import { onMount } from 'svelte';
	import { navigating, page } from '$app/state';
	import AppSidebar from '$lib/components/app-sidebar.svelte';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import { pageAvailable } from '$lib/nav';
	import { SERVER_DOCS } from '$lib/boot-failure';
	import { app } from '$lib/state/app.svelte';
	import favicon from '$website/public/favicon.png';

	let { children } = $props();

	const signingIn = $derived(page.url.pathname === '/login');

	// The current page stays while the next one loads its data; a slow switch shows a bar along the top.
	let waiting = $state(false);
	$effect(() => {
		if (!navigating.to) return void (waiting = false);
		const timer = setTimeout(() => (waiting = true), 120);
		return () => clearTimeout(timer);
	});

	onMount(() => {
		if (!signingIn) void app.start();
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

{#if waiting}<div class="nav-wait" role="progressbar" aria-label="Loading the page"></div>{/if}

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

<style>
	.nav-wait {
		position: fixed;
		inset: 0 0 auto;
		z-index: 50;
		height: 2px;
		background: var(--primary);
		transform-origin: left;
		animation: nav-wait 1.5s ease-out forwards;
	}
	@keyframes nav-wait {
		from {
			transform: scaleX(0.05);
		}
		to {
			transform: scaleX(0.9);
		}
	}
</style>
