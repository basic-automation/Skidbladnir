<script setup lang="ts">
const model = defineModel<boolean>({ required: true })

defineProps<{ label: string, help?: string, disabled?: boolean }>()
</script>

<template>
	<!-- The Nanna checkbox: a filled square, grey when off and accent when on, with no ring.
	     Disabled, only the square dims: Nuxt UI's 75% opacity on the whole root takes the
	     description under WCAG AA's 4.5:1 (3.03:1 in the app settings). -->
	<UCheckbox
		v-model="model"
		:disabled="disabled"
		:label="label"
		:description="help"
		class="min-w-0 px-2.5 py-[7px] text-left"
		:ui="{
			root: 'opacity-100',
			base: 'size-4 rounded-[4px] ring-0 bg-paleday-muted data-[state=checked]:bg-primary data-[disabled]:opacity-50',
			indicator: 'text-paleday-bg',
			wrapper: 'ms-2 flex flex-col gap-1',
			label: 'text-xs font-semibold text-paleday-fg',
			description: 'text-xs font-[450] text-paleday-dim',
		}"
	/>
</template>
