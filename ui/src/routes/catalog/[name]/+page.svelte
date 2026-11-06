<script lang="ts">
	import { page } from '$app/state';
	import NodeView from './node-view.svelte';

	const name = $derived(page.params.name ?? '');
</script>

{#key name}
	<NodeView {name} />
{/key}
