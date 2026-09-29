<script setup lang="ts">
// A file chosen from disk, by path: an ICC profile, an Exif or XMP payload.
import { isTauri } from '~/composables/useTauri'

const model = defineModel<string>({ required: true })

defineProps<{ label: string, help?: string }>()

async function choose() {
	const { open } = await import('@tauri-apps/plugin-dialog')
	const picked = await open({ multiple: false, directory: false })
	if (typeof picked === 'string') model.value = picked
}
</script>

<template>
	<div class="flex min-w-0 flex-col gap-2 px-2.5 py-[7px] text-xs">
		<span class="font-semibold text-paleday-fg">{{ label }}</span>
		<div class="flex items-center gap-2">
			<span class="min-w-0 flex-1 truncate text-paleday-bright" data-selectable>{{ model || 'No file chosen' }}</span>
			<UButton size="xs" color="neutral" variant="soft" :disabled="!isTauri()" :aria-label="`Choose a file for ${label}`" @click="choose">
				Choose…
			</UButton>
		</div>
		<p v-if="help" class="text-paleday-dim">
			{{ help }}
		</p>
	</div>
</template>
