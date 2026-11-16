<script lang="ts">
	import type { Snippet } from 'svelte';

	// A page's scrolling body. Every page scrolls in one of these. The scroller itself has no padding,
	// so a sticky header inside it sticks to its very top and no row shows through a gap above it;
	// the page's padding is on the box inside.
	let {
		padding = '20px 24px 40px',
		class: className = '',
		inner = '',
		scroller = $bindable(),
		onscroll,
		children
	}: {
		/** The page's padding, inside the scroller. */
		padding?: string;
		/** Classes for the scroller, e.g. to size it in a split view. */
		class?: string;
		/** Classes for the padded box inside. */
		inner?: string;
		scroller?: HTMLDivElement;
		onscroll?: (e: Event) => void;
		children: Snippet;
	} = $props();
</script>

<div class="page-body sb-scroll {className}" bind:this={scroller} {onscroll}>
	<div class="page-body-inner {inner}" style:padding>{@render children()}</div>
</div>

<style>
	.page-body {
		min-width: 0;
		min-height: 0;
		flex: 1;
		overflow: auto;
		padding: 0;
		scrollbar-color: var(--border-strong) transparent;
		scrollbar-width: thin;
	}
</style>
