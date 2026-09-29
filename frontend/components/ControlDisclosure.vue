<script setup lang="ts">
// A settings section that folds away: the expert options, which every reference tool has
// and most people never touch, behind their heading.
import { ref } from 'vue'

const props = defineProps<{ title: string, open?: boolean }>()
const expanded = ref(props.open ?? false)
</script>

<template>
	<UCollapsible v-model:open="expanded" class="flex w-full flex-col gap-4">
		<button type="button" data-disclosure class="flex w-full items-center gap-2 text-left text-sm font-semibold text-paleday-bright" :aria-expanded="expanded">
			<UIcon name="i-subway-down-2" class="size-4 shrink-0 transition-transform" :class="expanded ? '' : '-rotate-90'" />
			{{ title }}
		</button>
		<template #content>
			<div class="flex flex-col gap-4 pb-2">
				<slot />
			</div>
		</template>
	</UCollapsible>
</template>
