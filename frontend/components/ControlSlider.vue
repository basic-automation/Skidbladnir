<script setup lang="ts">
// A labelled slider with a live numeric readout — the job the Electron UI's `-info`
// spans did, kept in one place instead of wired up nine times.
const model = defineModel<number>({ required: true })

defineProps<{
	label: string
	min: number
	max: number
	step?: number
	help?: string
	disabled?: boolean
}>()
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
			<output class="tabular-nums" :class="disabled ? 'text-paleday-dim' : 'text-paleday-accent-text'" aria-live="off">{{ model }}</output>
		</div>
		<USlider
			v-model="model"
			:min="min"
			:max="max"
			:step="step ?? 1"
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
