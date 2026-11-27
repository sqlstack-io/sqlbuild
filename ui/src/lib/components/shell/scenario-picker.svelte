<script lang="ts">
	import { SCENARIO_NOTES, SYNC_SCENARIOS, type SyncScenario } from '$lib/api/mock/sync-scenarios';
	import { LOCATION_SHOP } from '$lib/api/mock/fixtures';
	import { to } from '$lib/paths';

	/** Mock only: switch the shop location between the sync-controls proposal's states, and jump to the pages that show them. */
	let { current }: { current: SyncScenario | null } = $props();
	let open = $state(true);

	function pick(scenario: string) {
		const search = new URLSearchParams(window.location.search);
		search.set('mock', '');
		if (scenario) search.set('scenario', scenario);
		else search.delete('scenario');
		window.location.search = search.toString().replace('mock=&', 'mock&').replace(/mock=$/, 'mock');
	}
	const pages = [
		{ label: 'Location', path: `/locations/${LOCATION_SHOP}` },
		{ label: 'Locations', path: '/locations' },
		{ label: 'Connections', path: '/connections' },
		{ label: 'Overview', path: '/' }
	];
</script>

<div class="picker" data-testid="scenario-picker">
	<!-- raw-button: a mock-only disclosure in the floating scenario picker, outside the app's controls -->
	<button class="head" onclick={() => (open = !open)} aria-expanded={open}>Mock scenario{current ? `: ${current}` : ''} {open ? '▾' : '▸'}</button>
	{#if open}
		<select value={current ?? ''} onchange={(e) => pick(e.currentTarget.value)} aria-label="Scenario">
			<option value="">None: the usual mock</option>
			{#each SYNC_SCENARIOS as s (s)}<option value={s}>{s}</option>{/each}
		</select>
		{#if current}<p class="note">{SCENARIO_NOTES[current]}</p>{/if}
		<nav>{#each pages as p (p.path)}<a href={to(p.path)}>{p.label}</a>{/each}</nav>
	{/if}
</div>

<style>
	.picker {
		position: fixed;
		right: 16px;
		bottom: 16px;
		z-index: 60;
		width: 280px;
		padding: 8px 10px;
		border: 1px solid var(--border);
		background: var(--background);
		box-shadow: 0 6px 24px rgb(0 0 0 / 0.14);
		font-size: 12px;
	}
	.head {
		width: 100%;
		text-align: left;
		font-weight: 600;
	}
	select {
		width: 100%;
		margin-top: 6px;
		padding: 3px 4px;
		border: 1px solid var(--input);
		background: var(--background);
		color: var(--foreground);
	}
	.note {
		margin-top: 6px;
		color: var(--muted-foreground);
	}
	nav {
		display: flex;
		gap: 10px;
		margin-top: 6px;
	}
	nav a {
		color: var(--primary);
	}
</style>
