<script lang="ts">
	import { page } from '$app/state';
	import CheckView from './check-view.svelte';

	const id = $derived(page.params.id ?? '');
</script>

{#key id}
	<CheckView {id} />
{/key}
