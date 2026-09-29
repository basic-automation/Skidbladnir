<script setup lang="ts">
// A labelled number, for values a slider cannot reach precisely: a byte count, a
// fractional quality, a code point. Clearing the field leaves the value as it was, since
// every number here is required; an option that can be left out is a ControlOptional.
import { computed } from 'vue'

const model = defineModel<number>({ required: true })

const props = withDefaults(defineProps<{
	label: string
	help?: string
	min?: number
	max?: number
	step?: number
	/** Digits after the point; 0 for whole numbers. */
	decimals?: number
	unit?: string
	disabled?: boolean
}>(), { step: 1, decimals: 0 })

const value = computed({
	get: () => model.value,
	set: (next: number | null) => {
		if (next !== null && Number.isFinite(next)) model.value = next
	},
})
const format = computed(() => ({ useGrouping: false, maximumFractionDigits: props.decimals }))
</script>

<template>
	<div class="flex min-w-0 flex-col gap-2 px-2.5 py-[7px] text-xs">
		<span class="font-semibold text-paleday-fg">{{ label }}</span>
		<div class="flex items-center gap-2">
			<UInputNumber
				v-model="value"
				:min="min"
				:max="max"
				:step="step"
				:step-snapping="false"
				:format-options="format"
				:disabled="disabled"
				:increment="false"
				:decrement="false"
				disable-wheel-change
				variant="soft"
				size="sm"
				:aria-label="label"
				class="min-w-0 flex-1"
				:ui="{ base: 'bg-paleday-rule text-paleday-fg hover:bg-paleday-rule focus:bg-paleday-rule tabular-nums' }"
			/>
			<span v-if="unit" class="shrink-0 text-paleday-dim">{{ unit }}</span>
		</div>
		<p v-if="help" class="text-paleday-dim">
			{{ help }}
		</p>
	</div>
</template>
