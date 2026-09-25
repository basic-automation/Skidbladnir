<script setup lang="ts">
// The conversion screen, following the Electron app's layout: a full-width dotted header
// band with a large centred title, dotted-bordered option groups laid out two to a row,
// and a circular accent FAB bottom-right for the action. Only the palette has moved, to
// Palenight, and the controls are Nuxt UI components skinned by the tokens in main.css.
//
// The show/hide behaviour is reproduced because it is real encoder behaviour: the advanced
// controls only reach the encoder in lossy mode, so offering them elsewhere would promise
// an effect that will not happen.
//
// Help text is libwebp's own wording from `cwebp -longhelp`. The Electron app was not a
// source for it: its `-info` spans are live value readouts and it has one tooltip in total.
import { computed, onMounted, onUnmounted, ref, toRaw, watch } from 'vue'
import type { ConversionReport, EncodeSettings, Mode, Preset } from '~/composables/useSettings'
import { formatBytes, usesLossyOptions, usesManualFilter } from '~/composables/useSettings'
import { invokeCommand, isTauri } from '~/composables/useTauri'

const settings = ref<EncodeSettings | null>(null)
const backendVersion = ref('')
const startupError = ref('')

const inputPaths = ref<string[]>([])
const outputDirectory = ref('')
const busy = ref(false)
const reports = ref<ConversionReport[]>([])
const failures = ref<{ path: string, message: string }[]>([])
const validationError = ref('')
const dragging = ref(false)
const dropRejected = ref(0)
let unlistenDrop: (() => void) | null = null

/** What the Rust core says about a path the user offered. */
interface WebpInfo { width: number, height: number, hasAlpha: boolean, hasAnimation: boolean, compression: 'lossy' | 'lossless' | 'mixed' }
interface PathInspection { path: string, supported: boolean, format: string | null, webp: WebpInfo | null }

const inspected = ref<PathInspection[]>([])

/** An image found by scanning a folder, with its position under the scan root. */
interface FoundImage { path: string, relative: string }
// Non-empty only when the selection came from a folder scan, which is what decides whether
// output mirrors the source tree or lands flat.
const scanned = ref<FoundImage[]>([])
const mirrorStructure = ref(true)
const animatedInputs = computed(() => inspected.value.filter(entry => entry.webp?.hasAnimation))

/** Preferences as the Rust side stores them, plus why it fell back if it did. */
interface Preferences { settings: EncodeSettings, outputDirectory: string | null }
interface LoadedPreferences { preferences: Preferences, fellBack: string | null }

const preferencesNotice = ref('')
const currentFile = ref('')
const currentPercent = ref(0)
const doneCount = ref(0)
const cancelling = ref(false)
const presets = ref<{ name: string, settings: EncodeSettings }[]>([])
const presetName = ref('')
const presetError = ref('')
let unlistenProgress: (() => void) | null = null
const FALLBACK_MESSAGES: Record<string, string> = {
	unreadable: 'Your saved settings could not be read, so the defaults were loaded.',
	unparseable: 'Your saved settings file was damaged, so the defaults were loaded.',
	invalidSettings: 'Your saved settings were out of range, so the defaults were loaded.',
}

const MODES: { value: Mode, label: string, help: string }[] = [
	{ value: 'lossy', label: 'Lossy', help: 'Ordinary WebP. The only mode with the advanced controls.' },
	{ value: 'lossless', label: 'Lossless', help: 'Exact pixels, larger files. Keeps colour under transparency.' },
	{ value: 'nearLossless', label: 'Near-lossless', help: 'Lossless with preprocessing; quality sets the level.' },
	{ value: 'jpegLike', label: 'JPEG-like', help: 'Roughly match the size a JPEG of this quality would be.' },
	{ value: 'preset', label: 'Preset', help: 'Use one of libwebp\'s tuned parameter sets.' },
]

const PRESET_ITEMS = [
	{ label: 'Select a preset', value: null },
	{ label: 'Default', value: 'default' },
	{ label: 'Photo', value: 'photo' },
	{ label: 'Picture', value: 'picture' },
	{ label: 'Drawing', value: 'drawing' },
	{ label: 'Icon', value: 'icon' },
	{ label: 'Text', value: 'text' },
] satisfies { label: string, value: Preset | null }[]

const FILTER_ITEMS = [
	{ label: 'Auto', value: 'auto' },
	{ label: 'Simple', value: 'simple' },
	{ label: 'Strong', value: 'strong' },
]

const TARGET_ITEMS = [
	{ label: 'Use quality', value: 'none' },
	{ label: 'Target file size', value: 'size' },
	{ label: 'Target PSNR', value: 'psnr' },
]

const lossy = computed(() => (settings.value ? usesLossyOptions(settings.value.mode) : false))
const manualFilter = computed(() => (settings.value ? usesManualFilter(settings.value.filter) : false))
const basicOnly = computed(() => settings.value?.mode === 'nearLossless' || settings.value?.mode === 'preset')

// The quality slider is relabelled per mode, because libwebp genuinely reinterprets it.
const qualityLabel = computed(() => {
	switch (settings.value?.mode) {
		case 'lossless': return 'Compression effort'
		case 'nearLossless': return 'Near-lossless level'
		default: return 'Quality'
	}
})
const qualityHelp = computed(() => {
	switch (settings.value?.mode) {
		case 'lossless': return '0 is fastest and largest, 100 slowest and smallest. Not fidelity here.'
		case 'nearLossless': return '0 is maximum loss, 100 is off.'
		default: return 'quality factor (0: small .. 100: big)'
	}
})

// Target size and PSNR are mutually exclusive in libwebp, so the UI offers one choice.
type TargetKind = 'none' | 'size' | 'psnr'
const targetKind = ref<TargetKind>('none')
const targetSize = ref(51200)
const targetPsnr = ref(42)

watch([targetKind, targetSize, targetPsnr], () => {
	if (!settings.value) return
	if (targetKind.value === 'size') settings.value.target = { kind: 'size', value: targetSize.value }
	else if (targetKind.value === 'psnr') settings.value.target = { kind: 'psnr', value: targetPsnr.value }
	else settings.value.target = null
})

onMounted(async () => {
	try {
		backendVersion.value = await invokeCommand<string>('encoder_version')
		// Preferences carry the defaults when there is nothing stored, so this is the only
		// place startup settings come from.
		const loaded = await invokeCommand<LoadedPreferences>('load_preferences')
		settings.value = loaded.preferences.settings
		outputDirectory.value = loaded.preferences.outputDirectory ?? ''
		// 'noFile' is a first launch, which is not worth telling anyone about.
		presets.value = await invokeCommand<{ name: string, settings: EncodeSettings }[]>('list_presets')
		if (loaded.fellBack && loaded.fellBack !== 'noFile') {
			preferencesNotice.value = FALLBACK_MESSAGES[loaded.fellBack] ?? 'Your saved settings could not be used, so the defaults were loaded.'
		}
	}
	catch (error) {
		startupError.value = error instanceof Error ? error.message : String(error)
	}

	if (!isTauri()) return

	const { listen } = await import('@tauri-apps/api/event')
	unlistenProgress = await listen<{ inputPath: string, percent: number }>('conversion-progress', (event) => {
		currentFile.value = event.payload.inputPath
		currentPercent.value = event.payload.percent
	})

	// Tauri's NATIVE drag-drop, not the HTML5 kind: with the native handler enabled the
	// webview's own dragover/drop events never fire, so listening for those would look
	// correct and silently do nothing.
	const { getCurrentWebview } = await import('@tauri-apps/api/webview')
	unlistenDrop = await getCurrentWebview().onDragDropEvent(async (event) => {
		if (event.payload.type === 'over') { dragging.value = true; return }
		if (event.payload.type === 'leave') { dragging.value = false; return }
		if (event.payload.type !== 'drop') return
		dragging.value = false

		// Which files are usable is decided by the Rust core, by reading each file's leading
		// bytes — never by matching extensions here, which would let the UI accept something
		// the loader then refuses.
		const dropped = await invokeCommand<PathInspection[]>('inspect_dropped_paths', { paths: event.payload.paths })
		const usable = dropped.filter(entry => entry.supported)
		dropRejected.value = dropped.length - usable.length
		if (usable.length > 0) {
			scanned.value = []
			inputPaths.value = usable.map(entry => entry.path)
			inspected.value = usable
		}
	})
})

onUnmounted(() => {
	unlistenDrop?.()
	unlistenProgress?.()
})

async function chooseInputs() {
	const { open } = await import('@tauri-apps/plugin-dialog')
	const picked = await open({ multiple: true, filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'jpe', 'jif', 'jfif', 'jfi', 'tif', 'tiff', 'webp'] }] })
	scanned.value = []
	if (Array.isArray(picked)) inputPaths.value = picked
	else if (typeof picked === 'string') inputPaths.value = [picked]
	await describeInputs()
}

/// Ask the core what the chosen files are, so the UI can describe them and warn about the
/// ones it cannot handle properly.
async function describeInputs() {
	if (inputPaths.value.length === 0) { inspected.value = []; return }
	try {
		inspected.value = await invokeCommand<PathInspection[]>('inspect_dropped_paths', { paths: inputPaths.value })
	}
	catch {
		inspected.value = []
	}
}

async function chooseFolder() {
	const { open } = await import('@tauri-apps/plugin-dialog')
	const picked = await open({ directory: true })
	if (typeof picked !== 'string') return
	scanned.value = await invokeCommand<FoundImage[]>('scan_folder', { directory: picked, recursive: true })
	inputPaths.value = scanned.value.map(entry => entry.path)
	await describeInputs()
}

async function chooseOutput() {
	const { open } = await import('@tauri-apps/plugin-dialog')
	const picked = await open({ directory: true })
	if (typeof picked === 'string') outputDirectory.value = picked
}

const canConvert = computed(() => {
	if (!settings.value || busy.value) return false
	if (inputPaths.value.length === 0 || !outputDirectory.value) return false
	return !(settings.value.mode === 'preset' && !settings.value.preset)
})

async function convert() {
	if (!settings.value) return
	busy.value = true
	cancelling.value = false
	reports.value = []
	failures.value = []
	validationError.value = ''
	doneCount.value = 0
	currentPercent.value = 0
	currentFile.value = ''
	try {
		// Validate once against the encoder's own rules rather than a copy of them here.
		await invokeCommand<EncodeSettings>('validate_settings', { settings: settings.value })
	}
	catch (error) {
		validationError.value = error instanceof Error ? error.message : String(error)
		busy.value = false
		return
	}

	// Sequential, so one failure is attributed to its own file and the rest still run.
	for (const input of inputPaths.value) {
		try {
			// A folder scan can reproduce the source tree in the destination; a flat
			// selection has no tree to reproduce.
			const found = scanned.value.find(entry => entry.path === input)
			const report = found && mirrorStructure.value
				? await invokeCommand<ConversionReport>('convert_scanned', { settings: settings.value, input, relative: found.relative, outputRoot: outputDirectory.value })
				: await invokeCommand<ConversionReport>('convert_image', { settings: settings.value, input, outputDirectory: outputDirectory.value })
			reports.value.push(report)
		}
		catch (error) {
			const message = error instanceof Error ? error.message : String(error)
			// A cancellation is what the user asked for, not a failure to report as one.
			if (message.includes('cancelled')) break
			failures.value.push({ path: input, message })
		}
		doneCount.value += 1
	}
	busy.value = false
	cancelling.value = false
	currentFile.value = ''

	// Remember what was used, after the run rather than on every slider drag: these are
	// settings that demonstrably got as far as the encoder.
	try {
		await invokeCommand<void>('save_preferences', { preferences: { settings: settings.value, outputDirectory: outputDirectory.value || null } })
	}
	catch {
		// Preferences are a convenience. Failing to store them must not look like a failed
		// conversion, which is what the user actually asked for and which succeeded.
	}
}

async function savePreset() {
	if (!settings.value) return
	presetError.value = ''
	try {
		presets.value = await invokeCommand('save_preset', { name: presetName.value, settings: settings.value })
		presetName.value = ''
	}
	catch (error) {
		presetError.value = error instanceof Error ? error.message : String(error)
	}
}

async function deletePreset(name: string) {
	presetError.value = ''
	try {
		presets.value = await invokeCommand('delete_preset', { name })
	}
	catch (error) {
		presetError.value = error instanceof Error ? error.message : String(error)
	}
}

function applyPreset(preset: { name: string, settings: EncodeSettings }) {
	// Copied, not aliased: editing the controls afterwards must not silently rewrite the
	// stored preset.
	settings.value = structuredClone(toRaw(preset.settings))
	// The target radio is UI state derived from settings.target, so bring it back in step.
	targetKind.value = settings.value.target?.kind ?? 'none'
	if (settings.value.target?.kind === 'size') targetSize.value = settings.value.target.value
	if (settings.value.target?.kind === 'psnr') targetPsnr.value = settings.value.target.value
}

async function cancel() {
	cancelling.value = true
	try {
		await invokeCommand<void>('cancel_conversion')
	}
	catch {
		cancelling.value = false
	}
}

const converted = computed(() => reports.value.length > 0 && failures.value.length === 0 && !busy.value)

function savingOf(report: ConversionReport): number {
	if (report.sourceBytes === 0) return 0
	return ((report.sourceBytes - report.outputBytes) / report.sourceBytes) * 100
}

function basename(path: string): string {
	return path.split(/[\\/]/).pop() ?? path
}
</script>

<template>
	<div class="flex min-h-full flex-col bg-palenight-bg">
		<!-- The old app's header: a full-width band with a dot texture and a large centred
		     title. White-on-black there; the Palenight surface here. -->
		<header class="dot-texture w-full border-b border-palenight-line bg-palenight-selection/60">
			<h1 class="py-10 text-center text-3xl font-extrabold tracking-tight text-palenight-bright">
				Skidbladnir
			</h1>
			<p class="-mt-6 pb-8 text-center text-sm text-palenight-fg">
				Convert images to next-generation formats, with the whole encoder exposed.
			</p>
		</header>

		<div
			v-if="dragging"
			class="pointer-events-none fixed inset-4 z-50 flex items-center justify-center rounded-lg border-2 border-dashed border-palenight-green bg-palenight-bg/85 text-lg font-semibold text-palenight-green"
		>
			Drop images to convert
		</div>

		<main class="mx-auto w-full max-w-5xl grow px-5 pt-8 pb-36">
			<UAlert
				v-if="preferencesNotice"
				color="warning"
				variant="subtle"
				:description="preferencesNotice"
				class="mb-6"
				close
				@update:open="preferencesNotice = ''"
			/>

			<UAlert
				v-if="startupError"
				color="warning"
				variant="subtle"
				title="Not connected to the encoder"
				:description="startupError"
				class="mb-6"
			/>

			<template v-if="settings">
				<div class="flex flex-col gap-5">
					<ControlPanel title="Files">
						<div class="grid gap-4 sm:grid-cols-2">
							<div class="text-left">
								<UButton color="neutral" variant="subtle" block :disabled="!isTauri()" @click="chooseInputs">
									Choose images…
								</UButton>
								<UButton color="neutral" variant="ghost" block size="xs" class="mt-1" :disabled="!isTauri()" @click="chooseFolder">
									…or a whole folder
								</UButton>
								<p class="mt-2 truncate text-xs text-palenight-fg" data-selectable>
									{{ inputPaths.length === 0 ? 'No files selected' : inputPaths.length === 1 ? basename(inputPaths[0]!) : `${inputPaths.length} files selected` }}
								</p>
							</div>
							<div class="text-left">
								<UButton color="neutral" variant="subtle" block :disabled="!isTauri()" @click="chooseOutput">
									Choose destination…
								</UButton>
								<p class="mt-2 truncate text-xs text-palenight-fg" data-selectable>
									{{ outputDirectory || 'No destination selected' }}
								</p>
							</div>
						</div>
						<div v-if="scanned.length > 0" class="flex flex-col items-center gap-2">
							<p class="text-xs text-palenight-cyan">
								Found {{ scanned.length }} image{{ scanned.length === 1 ? '' : 's' }} in that folder,
								including subfolders. Symlinks are skipped.
							</p>
							<ControlToggle v-model="mirrorStructure" label="Recreate the folder structure in the destination" help="Off writes every converted file side by side in the destination." />
						</div>
						<p v-if="inspected.length === 1 && inspected[0]?.webp" class="text-center text-xs text-palenight-cyan" data-selectable>
							Already a WebP: {{ inspected[0]!.webp!.width }}&times;{{ inspected[0]!.webp!.height }},
							{{ inspected[0]!.webp!.compression }}{{ inspected[0]!.webp!.hasAlpha ? ', with alpha' : '' }}.
						</p>
						<p v-if="animatedInputs.length > 0" class="text-center text-xs text-palenight-yellow">
							{{ animatedInputs.length === 1 ? 'One selected file is an animated WebP' : `${animatedInputs.length} selected files are animated WebPs` }}.
							Skidbladnir encodes still images, so {{ animatedInputs.length === 1 ? 'it' : 'they' }} will be
							skipped with an error rather than converted.
						</p>
						<p v-if="dropRejected > 0" class="text-center text-xs text-palenight-yellow">
							{{ dropRejected }} dropped {{ dropRejected === 1 ? 'file was' : 'files were' }} not a
							PNG, JPEG, TIFF or WebP and {{ dropRejected === 1 ? 'was' : 'were' }} skipped.
						</p>
						<p class="text-center text-xs text-palenight-comment">
							PNG, JPEG, TIFF and WebP — drop them anywhere on the window, or use the button.
							Converted files are written as <code class="text-palenight-cyan">&lt;name&gt;.webp</code>
							in the destination. Your originals are never written over.
						</p>
					</ControlPanel>

					<ControlPanel title="Mode">
						<div class="grid gap-3 sm:grid-cols-2" role="radiogroup" aria-label="Encoding mode">
							<button
								v-for="mode in MODES"
								:key="mode.value"
								type="button"
								role="radio"
								:aria-checked="settings.mode === mode.value"
								class="rounded-md border px-3 py-2 text-left transition-colors"
								:class="settings.mode === mode.value
									? 'border-palenight-green bg-palenight-green/10'
									: 'border-palenight-selection hover:border-palenight-comment'"
								@click="settings.mode = mode.value"
							>
								<span class="block text-sm font-semibold" :class="settings.mode === mode.value ? 'text-palenight-green' : 'text-palenight-bright'">{{ mode.label }}</span>
								<span class="block text-xs text-palenight-comment">{{ mode.help }}</span>
							</button>
						</div>

						<div v-if="settings.mode === 'preset'" class="mx-auto w-full max-w-sm text-left">
							<span class="text-sm font-medium text-palenight-bright">Preset</span>
							<USelect v-model="settings.preset" :items="PRESET_ITEMS" class="mt-1 w-full" />
							<p v-if="!settings.preset" class="mt-1 text-xs text-palenight-yellow">
								Pick a preset before converting.
							</p>
						</div>
					</ControlPanel>

					<ControlPanel title="My presets">
						<div v-if="presets.length" class="flex flex-wrap justify-center gap-2">
							<div v-for="preset in presets" :key="preset.name" class="flex items-center gap-1 rounded border border-palenight-selection pl-3">
								<button type="button" class="py-1 text-sm text-palenight-cyan hover:text-palenight-bright" @click="applyPreset(preset)">
									{{ preset.name }}
								</button>
								<button type="button" class="px-2 py-1 text-palenight-comment hover:text-palenight-red" :aria-label="`Delete preset ${preset.name}`" @click="deletePreset(preset.name)">
									<UIcon name="i-lucide-x" class="size-3.5" />
								</button>
							</div>
						</div>
						<p v-else class="text-center text-xs text-palenight-comment">
							No saved presets yet. Set the controls how you like them and give them a name.
						</p>
						<div class="mx-auto flex w-full max-w-md gap-2">
							<UInput v-model="presetName" placeholder="Name these settings…" aria-label="Name for the preset" class="flex-1" @keyup.enter="savePreset" />
							<UButton color="neutral" variant="subtle" :disabled="!presetName.trim()" @click="savePreset">
								Save
							</UButton>
						</div>
						<p v-if="presetError" class="text-center text-xs text-palenight-red">
							{{ presetError }}
						</p>
					</ControlPanel>

					<ControlPanel title="Quality">
						<div class="grid gap-5 sm:grid-cols-2">
							<ControlSlider v-model="settings.quality" :label="qualityLabel" :min="0" :max="100" :help="qualityHelp" />
							<ControlSlider v-model="settings.alphaQuality" label="Alpha quality" :min="0" :max="100" help="transparency-compression quality (0..100)" :disabled="basicOnly" />
							<ControlSlider v-model="settings.method" label="Compression method" :min="0" :max="6" help="0 = fast, 6 = slowest and smallest" :disabled="basicOnly" />
							<div class="grid grid-cols-2 gap-3 text-left">
								<div>
									<span class="text-sm font-medium text-palenight-bright">Resize width</span>
									<UInput v-model.number="settings.resize.width" type="number" :min="0" aria-label="Resize width in pixels" class="mt-1 w-full" />
								</div>
								<div>
									<span class="text-sm font-medium text-palenight-bright">Resize height</span>
									<UInput v-model.number="settings.resize.height" type="number" :min="0" aria-label="Resize height in pixels" class="mt-1 w-full" />
								</div>
								<p class="col-span-2 text-xs text-palenight-comment">
									Applied before encoding. Both 0 means no resize; set one to 0 to derive it
									and keep the aspect ratio.
								</p>
							</div>
						</div>
					</ControlPanel>

					<!-- The Electron app kept the expert controls behind a disclosure. -->
					<ControlPanel v-if="lossy" title="Advanced options">
						<div class="grid gap-5 sm:grid-cols-2">
							<ControlSlider v-model="settings.sns" label="Spatial noise shaping" :min="0" :max="100" help="0 = off, 100 = maximum" />
							<ControlSlider v-model="settings.segments" label="Segments" :min="1" :max="4" help="number of segments to use (1..4)" />
							<ControlSlider v-model="settings.partitionLimit" label="Partition limit" :min="0" :max="100" help="degradation allowed to fit the 512k prediction-mode limit" />
							<ControlSlider v-model="settings.passes" label="Analysis passes" :min="1" :max="10" help="analysis pass number (1..10)" />
						</div>

						<div class="grid gap-5 border-t border-dotted border-palenight-selection pt-5 sm:grid-cols-2">
							<div class="text-left">
								<span class="text-sm font-medium text-palenight-bright">Deblocking filter</span>
								<URadioGroup v-model="settings.filter" :items="FILTER_ITEMS" orientation="horizontal" class="mt-2" />
								<p class="mt-1 text-xs text-palenight-comment">
									Auto lets the encoder pick the strength, so the two sliders do not apply.
								</p>
							</div>
							<div class="flex flex-col gap-5">
								<ControlSlider v-model="settings.filterStrength" label="Filter strength" :min="0" :max="100" help="0 = off .. 100 = strongest" :disabled="!manualFilter" />
								<ControlSlider v-model="settings.filterSharpness" label="Filter sharpness" :min="0" :max="7" help="0 = most sharp .. 7 = least sharp" :disabled="!manualFilter" />
							</div>
						</div>

						<div class="grid gap-5 border-t border-dotted border-palenight-selection pt-5 sm:grid-cols-2">
							<div class="text-left">
								<span class="text-sm font-medium text-palenight-bright">Compression target</span>
								<URadioGroup v-model="targetKind" :items="TARGET_ITEMS" class="mt-2" />
								<p class="mt-1 text-xs text-palenight-comment">
									A size or PSNR target overrides the quality slider.
								</p>
							</div>
							<div class="flex flex-col gap-5">
								<div v-if="targetKind === 'size'" class="text-left">
									<span class="text-sm font-medium text-palenight-bright">Target size (bytes)</span>
									<UInput v-model.number="targetSize" type="number" :min="1" aria-label="Target size in bytes" class="mt-1 w-full" />
								</div>
								<ControlSlider v-if="targetKind === 'psnr'" v-model="targetPsnr" label="Target PSNR (dB)" :min="1" :max="10000" help="typically around 42" />
							</div>
						</div>

						<div class="grid gap-3 border-t border-dotted border-palenight-selection pt-5 sm:grid-cols-3">
							<ControlToggle v-model="settings.sharpYuv" label="Sharp YUV" help="sharper (and slower) RGB to YUV" />
							<ControlToggle v-model="settings.lowMemory" label="Low memory" help="less memory, slower encoding" />
							<ControlToggle v-model="settings.multiThreading" label="Multi-threading" help="use multi-threading if available" />
						</div>
					</ControlPanel>

					<ControlPanel v-if="busy" title="Converting">
						<div class="text-left" role="status" aria-live="polite">
							<div class="flex items-baseline justify-between gap-3">
								<span class="truncate text-sm text-palenight-bright" data-selectable>
									{{ currentFile ? basename(currentFile) : 'Starting…' }}
								</span>
								<span class="font-mono text-sm tabular-nums text-palenight-green">
									{{ doneCount }} / {{ inputPaths.length }}
								</span>
							</div>
							<UProgress v-model="currentPercent" :max="100" class="mt-2" />
							<p class="mt-1 text-xs text-palenight-comment">
								libwebp does not always report a final 100%, so the bar can stop short of
								the end before a file finishes.
							</p>
						</div>
						<div class="flex justify-center">
							<UButton color="error" variant="subtle" :loading="cancelling" @click="cancel">
								{{ cancelling ? 'Stopping…' : 'Cancel' }}
							</UButton>
						</div>
					</ControlPanel>

					<UAlert v-if="validationError" color="error" variant="subtle" role="alert" :description="validationError" />

					<ControlPanel v-if="reports.length || failures.length" title="Results">
						<p class="sr-only" role="status" aria-live="polite">
							{{ reports.length }} converted, {{ failures.length }} failed.
						</p>
						<table v-if="reports.length" class="w-full text-left text-sm">
							<thead class="text-xs tracking-wide text-palenight-comment uppercase">
								<tr>
									<th class="py-1">File</th>
									<th class="py-1 text-right">Before</th>
									<th class="py-1 text-right">After</th>
									<th class="py-1 text-right">Change</th>
									<th class="py-1 text-right">Size</th>
								</tr>
							</thead>
							<tbody class="font-mono tabular-nums text-palenight-fg">
								<tr v-for="report in reports" :key="report.outputPath" class="border-t border-palenight-selection">
									<td class="py-1 pr-2 font-sans text-palenight-bright" data-selectable>{{ basename(report.outputPath) }}</td>
									<td class="py-1 text-right">{{ formatBytes(report.sourceBytes) }}</td>
									<td class="py-1 text-right">{{ formatBytes(report.outputBytes) }}</td>
									<td class="py-1 text-right" :class="savingOf(report) >= 0 ? 'text-palenight-green' : 'text-palenight-orange'">
										{{ savingOf(report) >= 0 ? '−' : '+' }}{{ Math.abs(savingOf(report)).toFixed(1) }}%
									</td>
									<td class="py-1 text-right">{{ report.width }}×{{ report.height }}</td>
								</tr>
							</tbody>
						</table>
						<ul v-if="failures.length" class="flex flex-col gap-1 text-left text-sm text-palenight-red">
							<li v-for="failure in failures" :key="failure.path" data-selectable>
								<strong class="text-palenight-bright">{{ basename(failure.path) }}</strong>: {{ failure.message }}
							</li>
						</ul>
					</ControlPanel>
				</div>
			</template>
		</main>

		<!-- The old app's fixed dot-textured footer band, with the version readout it never
		     had but which belongs somewhere unobtrusive. -->
		<footer class="dot-texture pointer-events-none fixed inset-x-0 bottom-0 h-24 border-t border-palenight-line bg-palenight-panel/90">
			<p v-if="backendVersion" class="px-5 pt-3 font-mono text-[11px] text-palenight-comment" data-selectable>
				{{ backendVersion }}
			</p>
		</footer>

		<!-- The circular accent FAB, bottom-right, exactly where the Electron app put it. -->
		<button
			type="button"
			class="fixed right-12 bottom-12 z-[100] flex size-[70px] items-center justify-center rounded-full text-3xl shadow-lg transition-transform"
			:class="canConvert
				? 'cursor-pointer bg-palenight-green text-palenight-bg hover:scale-105'
				: 'cursor-not-allowed bg-palenight-selection text-palenight-comment'"
			:disabled="!canConvert"
			:title="canConvert ? 'Convert' : 'Choose images and a destination first'"
			:aria-label="busy ? 'Converting' : 'Convert'"
			@click="convert"
		>
			<UIcon v-if="busy" name="i-lucide-loader-circle" class="size-8 animate-spin" />
			<UIcon v-else-if="converted" name="i-lucide-check" class="size-8" />
			<UIcon v-else name="i-lucide-arrow-right" class="size-8" />
		</button>
	</div>
</template>
