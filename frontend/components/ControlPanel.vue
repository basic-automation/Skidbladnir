<script setup lang="ts">
// A settings section in the Nanna layout: a plain heading over its controls, with no
// frame around them. Grouping is carried by spacing alone. The `actions` slot sits at the
// end of the heading row, for a section that can be dismissed or acted on as a whole.
defineProps<{ title: string }>()
</script>

<template>
	<section class="flex w-full flex-col gap-4">
		<div class="flex items-center gap-3">
			<h2 class="min-w-0 flex-1 text-sm font-semibold text-paleday-bright">
				{{ title }}
			</h2>
			<slot name="actions" />
		</div>
		<slot />
	</section>
</template>
