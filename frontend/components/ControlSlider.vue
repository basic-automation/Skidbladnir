<script setup lang="ts">
// A labelled slider with a live numeric readout — the job the Electron UI's `-info`
// spans did, kept in one place instead of wired up nine times. With `decimals`, the
// readout is a field too, so a fractional value (`cwebp -q 75.3`, `cjxl -d 0.8`) can be
// typed exactly rather than only reached in slider steps.
import { computed } from 'vue'

const model = defineModel<number>({ required: true })

const props = withDefaults(defineProps<{
	label: string
	min: number
	max: number
	step?: number
	/** Digits after the point the typed value may carry; unset for a plain readout. */
	decimals?: number
	help?: string
	disabled?: boolean
	/** Show the value divided by this, to one decimal: a setting held in tenths reads as 1.0, not 10. */
	divisor?: number
}>(), { step: 1 })

const typed = computed({
	get: () => model.value,
	set: (next: number | null) => {
		if (next !== null && Number.isFinite(next)) model.value = Math.min(props.max, Math.max(props.min, next))
	},
})
const format = computed(() => ({ useGrouping: false, maximumFractionDigits: props.decimals ?? 0 }))
</script>

<template>
	<!-- A disabled control dims the SLIDER, not its text: fading the label and help would
	     take them under AA contrast. The affordance is carried by the greyed track and the
	     "not in use" note instead. -->
	<div class="flex min-w-0 flex-col gap-2 px-2.5 py-[7px] text-left">
		<div class="flex items-baseline gap-3 text-xs font-semibold">
			<span class="min-w-0 flex-1 text-paleday-fg">
				{{ label }}<span v-if="disabled" class="font-[450] text-paleday-dim"> · not in use</span>
			</span>
			<UInputNumber
				v-if="decimals !== undefined"
				v-model="typed"
				:min="min"
				:max="max"
				:step="step"
				:step-snapping="false"
				:format-options="format"
				:disabled="disabled"
				:increment="false"
				:decrement="false"
				disable-wheel-change
				variant="none"
				size="xs"
				:aria-label="`${label}, exact value`"
				class="w-16"
				:ui="{ base: 'p-0 text-right text-xs font-semibold tabular-nums text-paleday-accent-text' }"
			/>
			<output v-else class="tabular-nums" :class="disabled ? 'text-paleday-dim' : 'text-paleday-accent-text'" aria-live="off">{{ divisor ? (model / divisor).toFixed(1) : model }}</output>
		</div>
		<USlider
			v-model="model"
			:min="min"
			:max="max"
			:step="step"
			:disabled="disabled"
			size="sm"
			:aria-label="label"
			:class="disabled && 'opacity-50'"
			:ui="{ track: 'bg-paleday-dim', thumb: 'bg-paleday-bg ring-primary' }"
		/>
		<p v-if="help" class="text-xs text-paleday-dim">
			{{ help }}
		</p>
	</div>
</template>
