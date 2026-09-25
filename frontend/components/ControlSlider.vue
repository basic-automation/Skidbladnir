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
	<!-- A disabled control dims the SLIDER, not its text. Fading the whole block to 40%
	     took the label and help text to 2.6:1 and 2.0:1 against the panel, which axe-core
	     flagged and which is simply unreadable; the affordance is carried by the greyed
	     track and the "not in use" note instead. -->
	<div class="text-left">
		<div class="flex items-baseline justify-between gap-3">
			<span class="text-sm font-medium" :class="disabled ? 'text-palenight-muted' : 'text-palenight-bright'">
				{{ label }}<span v-if="disabled" class="font-normal"> · not in use</span>
			</span>
			<output class="font-mono text-sm tabular-nums" :class="disabled ? 'text-palenight-muted' : 'text-palenight-green'" aria-live="off">{{ model }}</output>
		</div>
		<USlider
			v-model="model"
			class="mt-2"
			:min="min"
			:max="max"
			:step="step ?? 1"
			:disabled="disabled"
			size="sm"
			:aria-label="label"
			:class="disabled && 'opacity-50'"
		/>
		<p v-if="help" class="mt-1 text-xs text-palenight-muted">
			{{ help }}
		</p>
	</div>
</template>
