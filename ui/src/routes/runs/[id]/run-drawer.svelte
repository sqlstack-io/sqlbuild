<script lang="ts">
	import XIcon from '@lucide/svelte/icons/x';
	import type { Snippet } from 'svelte';
	import Button from '$lib/components/ui/button/button.svelte';

	// The run page's right-side panel: beside the page (pushing it aside, so nothing under it is clipped),
	// resizable from its left edge, closed with ✕ or Esc. The node drawer and Details both use it.
	let {
		label,
		testid,
		onclose,
		header,
		actions,
		children,
		width = $bindable(480)
	}: {
		label: string;
		testid?: string;
		onclose: () => void;
		/** The title and anything under it. */
		header: Snippet;
		/** Links beside ✕. */
		actions?: Snippet;
		children: Snippet;
		width?: number;
	} = $props();

	let dragging = $state(false);
	const clampWidth = (w: number) => Math.round(Math.min(Math.max(340, w), Math.max(360, window.innerWidth * 0.6)));
	function onkey(e: KeyboardEvent) {
		if (e.key === 'Escape' && !e.defaultPrevented) onclose();
	}
</script>

<svelte:window onkeydown={onkey} />

<aside class="drawer" style:width="{width}px" aria-label={label} data-testid={testid}>
	<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
	<div
		class="resize"
		role="separator"
		aria-orientation="vertical"
		aria-label="Resize the panel"
		aria-valuenow={width}
		tabindex="0"
		onpointerdown={(e) => {
			dragging = true;
			(e.target as HTMLElement).setPointerCapture(e.pointerId);
		}}
		onpointermove={(e) => dragging && (width = clampWidth(window.innerWidth - e.clientX))}
		onpointerup={() => (dragging = false)}
		onkeydown={(e) => {
			if (e.key === 'ArrowLeft') width = clampWidth(width + 24);
			if (e.key === 'ArrowRight') width = clampWidth(width - 24);
		}}
	></div>
	<header>
		<div class="top">
			<div class="grow">{@render header()}</div>
			<div class="actions">
				{@render actions?.()}
				<Button variant="ghost" size="icon-sm" onclick={onclose} aria-label="Close" title="Close (Esc)"><XIcon /></Button>
			</div>
		</div>
	</header>
	<div class="body">{@render children()}</div>
</aside>

<style>
	.drawer {
		position: relative;
		display: flex;
		min-height: 0;
		flex: none;
		flex-direction: column;
		border-left: 1px solid var(--border-subtle);
		background: var(--card);
		font-size: 12.5px;
	}
	.resize {
		position: absolute;
		top: 0;
		bottom: 0;
		left: -3px;
		z-index: 2;
		width: 6px;
		cursor: col-resize;
	}
	.resize:hover,
	.resize:focus-visible {
		background: color-mix(in srgb, var(--primary) 35%, transparent);
	}
	header {
		padding: 14px 16px 12px;
		border-bottom: 1px solid var(--border-subtle);
	}
	.top {
		display: flex;
		align-items: flex-start;
		gap: 10px;
	}
	.grow {
		min-width: 0;
		flex: 1;
	}
	.actions {
		display: flex;
		align-items: center;
		gap: 12px;
		font-size: 12px;
	}
	.body {
		overflow-x: hidden;
		overflow-y: auto;
		flex: 1;
		padding: 4px 8px 24px;
	}
</style>
