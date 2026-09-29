<script setup lang="ts">
// A colour as the 0xRRGGBB number cwebp -blend_alpha takes.
import { computed } from 'vue'

const model = defineModel<number>({ required: true })

defineProps<{ label: string, help?: string }>()

const hex = computed({
	get: () => `#${model.value.toString(16).padStart(6, '0')}`,
	set: (next: string) => {
		const parsed = Number.parseInt(next.replace('#', ''), 16)
		if (Number.isFinite(parsed)) model.value = parsed & 0xffffff
	},
})
</script>

<template>
	<div class="flex min-w-0 flex-col gap-2 px-2.5 py-[7px] text-xs">
		<span class="font-semibold text-paleday-fg">{{ label }}</span>
		<div class="flex items-center gap-2">
			<input v-model="hex" type="color" :aria-label="label" class="size-7 cursor-pointer rounded-md bg-transparent">
			<span class="font-mono tabular-nums text-paleday-bright" data-selectable>0x{{ hex.slice(1) }}</span>
		</div>
		<p v-if="help" class="text-paleday-dim">
			{{ help }}
		</p>
	</div>
</template>
