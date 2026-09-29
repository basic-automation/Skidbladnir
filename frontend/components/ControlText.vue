<script setup lang="ts">
const model = defineModel<string>({ required: true })

defineProps<{ label: string, help?: string, placeholder?: string, disabled?: boolean }>()
</script>

<template>
	<div class="flex min-w-0 flex-col gap-2 px-2.5 py-[7px] text-xs">
		<span class="font-semibold text-paleday-fg">{{ label }}</span>
		<UInput v-model="model" :placeholder="placeholder" :disabled="disabled" variant="soft" size="sm" :aria-label="label" :ui="{ base: 'bg-paleday-rule text-paleday-fg hover:bg-paleday-rule focus:bg-paleday-rule' }" />
		<p v-if="help" class="text-paleday-dim">
			{{ help }}
		</p>
	</div>
</template>
