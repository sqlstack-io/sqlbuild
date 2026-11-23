<script lang="ts">
	// Mockup (`?cards=new`): one-click fonts for the graph cards' names and status lines, kept in the
	// URL as `font=` so a link reproduces the choice. Text fitted to the cards is measured again on a change.
	import { onDestroy, untrack } from 'svelte';
	import { page } from '$app/state';
	import { replaceState } from '$app/navigation';
	import { fonts } from './fonts.svelte';

	type Option = { key: string; label: string; card: string | null; pill: string | null; load?: () => Promise<unknown> };
	const OPTIONS: Option[] = [
		// Today's and `cards=new`'s cards: Inter on the card, IBM Plex Mono in the zoomed-out pill.
		{ key: 'current', label: 'New (current)', card: null, pill: null },
		{ key: 'inter', label: 'Inter', card: "'Inter Variable', sans-serif", pill: "'Inter Variable', sans-serif" },
		{ key: 'space-grotesk', label: 'Space Grotesk', card: "'Space Grotesk Variable', sans-serif", pill: "'Space Grotesk Variable', sans-serif" },
		{ key: 'plex-mono', label: 'IBM Plex Mono', card: "'IBM Plex Mono', monospace", pill: "'IBM Plex Mono', monospace" },
		{ key: 'plex-sans', label: 'IBM Plex Sans', card: "'IBM Plex Sans', sans-serif", pill: "'IBM Plex Sans', sans-serif", load: () => Promise.all([import('@fontsource/ibm-plex-sans/500.css'), import('@fontsource/ibm-plex-sans/600.css')]) },
		{ key: 'jetbrains-mono', label: 'JetBrains Mono', card: "'JetBrains Mono', monospace", pill: "'JetBrains Mono', monospace", load: () => Promise.all([import('@fontsource/jetbrains-mono/500.css'), import('@fontsource/jetbrains-mono/600.css')]) },
		{ key: 'geist', label: 'Geist', card: "'Geist Sans', sans-serif", pill: "'Geist Sans', sans-serif", load: () => Promise.all([import('@fontsource/geist-sans/500.css'), import('@fontsource/geist-sans/600.css')]) },
		{ key: 'geist-mono', label: 'Geist Mono', card: "'Geist Mono', monospace", pill: "'Geist Mono', monospace", load: () => Promise.all([import('@fontsource/geist-mono/500.css'), import('@fontsource/geist-mono/600.css')]) },
		{ key: 'system', label: 'system-ui', card: 'system-ui, sans-serif', pill: 'system-ui, sans-serif' }
	];

	const initial = page.url.searchParams.get('font');
	let key = $state(OPTIONS.some((o) => o.key === initial) ? initial! : 'current');
	const current = $derived(OPTIONS.find((o) => o.key === key)!);
	let open = $state(false);

	function apply(option: Option) {
		const root = document.documentElement.style;
		if (option.card) root.setProperty('--card-font', option.card);
		else root.removeProperty('--card-font');
		if (option.pill) root.setProperty('--font-node-compact', option.pill);
		else root.removeProperty('--font-node-compact');
		// Fit the names and status lines again in the new font, now and once its faces have loaded.
		fonts.version++;
		const family = (option.card ?? '').split(',')[0];
		void (option.load?.() ?? Promise.resolve())
			.then(() => (family ? Promise.all([document.fonts?.load(`600 16px ${family}`), document.fonts?.load(`500 16px ${family}`)]) : null))
			.then(() => fonts.version++)
			.catch(() => {});
	}
	$effect(() => {
		const option = current;
		untrack(() => apply(option));
	});
	onDestroy(() => apply(OPTIONS[0]));

	function choose(next: string) {
		key = next;
		open = false;
		const url = new URL(page.url);
		if (next === 'current') url.searchParams.delete('font');
		else url.searchParams.set('font', next);
		replaceState(url, page.state);
	}
</script>

<div class="fonts nodrag nopan" role="group" aria-label="Card font">
	{#if open}
		<ul>
			{#each OPTIONS as o (o.key)}
				<li>
					<!-- raw-button: a mockup's font choice, floating on the graph canvas -->
					<button class:on={o.key === key} style:font-family={o.card ?? "'Inter Variable', sans-serif"} onclick={() => choose(o.key)}>{o.label}</button>
				</li>
			{/each}
		</ul>
	{/if}
	<!-- raw-button: opens the mockup's font choices -->
	<button class="head" aria-expanded={open} onclick={() => (open = !open)}><span class="k">Card font</span> {current.label}</button>
</div>

<style>
	.fonts {
		position: absolute;
		right: 12px;
		bottom: 12px;
		z-index: 10;
		display: flex;
		flex-direction: column;
		align-items: stretch;
		min-width: 180px;
		border: 1px solid var(--border);
		border-radius: 3px;
		background: var(--card);
		font-size: 12px;
		box-shadow: 0 4px 16px rgb(0 0 0 / 0.25);
	}
	ul {
		margin: 0;
		padding: 4px;
		list-style: none;
		border-bottom: 1px solid var(--border);
	}
	ul button {
		width: 100%;
		padding: 4px 8px;
		border-radius: 2px;
		color: var(--foreground);
		font-size: 13px;
		text-align: left;
	}
	ul button:hover {
		background: var(--sb-hover);
	}
	ul button.on {
		background: color-mix(in srgb, var(--primary) 16%, transparent);
		color: var(--primary);
	}
	.head {
		padding: 6px 10px;
		color: var(--foreground);
		text-align: left;
	}
	.head:hover {
		background: var(--sb-hover);
	}
	.k {
		margin-right: 6px;
		color: var(--muted-foreground);
	}
</style>
