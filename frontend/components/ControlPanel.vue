<script setup lang="ts">
// The Electron app's option groups were blocks with a 2px DOTTED border and a centred
// layout. That dotted border is the most recognisable thing about the old UI, so it is
// kept rather than replaced with the flat card every app has.
defineProps<{ title: string }>()
</script>

<template>
	<section class="rounded-md border-2 border-dotted border-palenight-comment/70 bg-palenight-panel/40 p-5">
		<h2 class="mb-4 text-center text-xs font-bold tracking-[0.2em] text-palenight-purple uppercase">
			{{ title }}
		</h2>
		<div class="flex flex-col gap-5">
			<slot />
		</div>
	</section>
</template>
