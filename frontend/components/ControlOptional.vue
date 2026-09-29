<script setup lang="ts" generic="T">
// An option that can be left out, which is not the same as giving it any value: avifenc's
// quality when not given lets --target-size search it, cjxl's alpha distance when not
// given is lossless, heif-enc writes no thumbnail at all. The switch gives or removes the
// option; the controls for its value, in the slot, appear only while it is given.
import { computed, toRaw } from 'vue'

const model = defineModel<T | null>({ required: true })

const props = defineProps<{
	label: string
	help?: string
	/** The value the option takes when it is switched on. */
	fallback: T
	/** What leaving it out means, shown while it is off. */
	unset?: string
}>()

const given = computed({
	get: () => model.value !== null,
	set: (on: boolean) => {
		const fallback = toRaw(props.fallback)
		model.value = on ? (typeof fallback === 'object' && fallback !== null ? structuredClone(fallback) : fallback) : null
	},
})
</script>

<template>
	<div class="flex min-w-0 flex-col">
		<ControlToggle v-model="given" :label="label" :help="given || !unset ? help : unset" />
		<div v-if="model !== null" class="flex flex-col pl-5">
			<slot :value="model" />
		</div>
	</div>
</template>
