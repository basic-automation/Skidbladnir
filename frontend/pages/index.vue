<script setup lang="ts">
// The conversion screen. Every control the Electron app exposed is here, with the same
// ranges, the same grouping and the same show/hide behaviour — that behaviour is real
// (the advanced controls only reach the encoder in lossy mode), so hiding them elsewhere
// is honesty rather than tidiness.
//
// Help text is taken from `cwebp -longhelp`, i.e. libwebp's own wording. The Electron app
// was not a source for it: its `-info` spans are live value readouts, and it has exactly
// one tooltip in the whole UI.
import { computed, onMounted, ref, watch } from 'vue'
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

const MODES: { value: Mode, label: string, help: string }[] = [
	{ value: 'lossy', label: 'Lossy', help: 'Ordinary WebP. The only mode with the advanced controls.' },
	{ value: 'lossless', label: 'Lossless', help: 'Exact pixels, larger files. Preserves colour under transparency.' },
	{ value: 'nearLossless', label: 'Near-lossless', help: 'Lossless with preprocessing; the quality slider sets the level.' },
	{ value: 'jpegLike', label: 'JPEG-like', help: 'Roughly match the size a JPEG of this quality would be.' },
	{ value: 'preset', label: 'Preset', help: 'Use one of libwebp\'s tuned parameter sets.' },
]

const PRESETS: Preset[] = ['default', 'photo', 'picture', 'drawing', 'icon', 'text']

const lossy = computed(() => (settings.value ? usesLossyOptions(settings.value.mode) : false))
const manualFilter = computed(() => (settings.value ? usesManualFilter(settings.value.filter) : false))

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
		case 'lossless': return '0 is fastest and largest, 100 is slowest and smallest. Not a fidelity control here.'
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
		settings.value = await invokeCommand<EncodeSettings>('default_settings')
	}
	catch (error) {
		startupError.value = error instanceof Error ? error.message : String(error)
	}
})

async function chooseInputs() {
	const { open } = await import('@tauri-apps/plugin-dialog')
	const picked = await open({ multiple: true, filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'jpe', 'jif', 'jfif', 'jfi', 'tif', 'tiff', 'webp'] }] })
	if (Array.isArray(picked)) inputPaths.value = picked
	else if (typeof picked === 'string') inputPaths.value = [picked]
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
	reports.value = []
	failures.value = []
	validationError.value = ''
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
			reports.value.push(await invokeCommand<ConversionReport>('convert_image', { settings: settings.value, input, outputDirectory: outputDirectory.value }))
		}
		catch (error) {
			failures.value.push({ path: input, message: error instanceof Error ? error.message : String(error) })
		}
	}
	busy.value = false
}

function saving(report: ConversionReport): string {
	if (report.sourceBytes === 0) return ''
	const percent = ((report.sourceBytes - report.outputBytes) / report.sourceBytes) * 100
	return `${percent >= 0 ? '-' : '+'}${Math.abs(percent).toFixed(1)}%`
}

function basename(path: string): string {
	return path.split(/[\\/]/).pop() ?? path
}
</script>

<template>
	<main class="mx-auto flex min-h-full max-w-5xl flex-col gap-5 p-6 text-slate-900 dark:text-slate-100">
		<header class="flex items-baseline justify-between gap-4">
			<div>
				<h1 class="text-2xl font-semibold tracking-tight">
					Skidbladnir
				</h1>
				<p class="text-sm text-slate-600 dark:text-slate-400">
					Convert images to WebP, with the whole encoder exposed.
				</p>
			</div>
			<p v-if="backendVersion" class="font-mono text-xs text-slate-500" data-selectable>
				{{ backendVersion }}
			</p>
		</header>

		<p v-if="startupError" class="rounded-lg border border-amber-300 bg-amber-50 p-3 text-sm text-amber-800 dark:border-amber-800 dark:bg-amber-950 dark:text-amber-200">
			{{ startupError }}
		</p>

		<template v-if="settings">
			<ControlPanel title="Files">
				<div class="flex flex-wrap items-center gap-3">
					<button type="button" class="rounded border border-slate-300 px-3 py-1.5 text-sm font-medium hover:bg-slate-100 dark:border-slate-700 dark:hover:bg-slate-800" :disabled="!isTauri()" @click="chooseInputs">
						Choose images&hellip;
					</button>
					<span class="text-sm text-slate-600 dark:text-slate-400">
						{{ inputPaths.length === 0 ? 'No files selected' : `${inputPaths.length} file${inputPaths.length === 1 ? '' : 's'} selected` }}
					</span>
				</div>
				<div class="flex flex-wrap items-center gap-3">
					<button type="button" class="rounded border border-slate-300 px-3 py-1.5 text-sm font-medium hover:bg-slate-100 dark:border-slate-700 dark:hover:bg-slate-800" :disabled="!isTauri()" @click="chooseOutput">
						Choose destination&hellip;
					</button>
					<span class="truncate text-sm text-slate-600 dark:text-slate-400" data-selectable>
						{{ outputDirectory || 'No destination selected' }}
					</span>
				</div>
				<p class="text-xs text-slate-500 dark:text-slate-400">
					PNG, JPEG, TIFF and WebP. Converted files are written as
					<code>&lt;name&gt;.webp</code> in the destination. Your originals are never
					written over.
				</p>
			</ControlPanel>

			<ControlPanel title="Mode">
				<div class="grid gap-2 sm:grid-cols-2">
					<label v-for="mode in MODES" :key="mode.value" class="flex gap-2 rounded border border-transparent p-1 hover:border-slate-200 dark:hover:border-slate-800">
						<input v-model="settings.mode" type="radio" name="mode" :value="mode.value" class="mt-0.5 size-4 shrink-0 accent-sky-600">
						<span>
							<span class="text-sm font-medium">{{ mode.label }}</span>
							<span class="block text-xs text-slate-500 dark:text-slate-400">{{ mode.help }}</span>
						</span>
					</label>
				</div>

				<label v-if="settings.mode === 'preset'" class="block">
					<span class="text-sm font-medium">Preset</span>
					<select v-model="settings.preset" class="mt-1 block w-full rounded border border-slate-300 bg-white p-1.5 text-sm dark:border-slate-700 dark:bg-slate-900">
						<option :value="null">Select a preset</option>
						<option v-for="preset in PRESETS" :key="preset" :value="preset">{{ preset }}</option>
					</select>
					<span v-if="!settings.preset" class="mt-0.5 block text-xs text-amber-600 dark:text-amber-400">Pick a preset before converting.</span>
				</label>
			</ControlPanel>

			<ControlPanel title="Quality">
				<ControlSlider v-model="settings.quality" :label="qualityLabel" :min="0" :max="100" :help="qualityHelp" />
				<ControlSlider v-model="settings.alphaQuality" label="Alpha quality" :min="0" :max="100" help="transparency-compression quality (0..100)" :disabled="settings.mode === 'nearLossless' || settings.mode === 'preset'" />
				<ControlSlider v-model="settings.method" label="Compression method" :min="0" :max="6" help="0 = fast, 6 = slowest and smallest" :disabled="settings.mode === 'nearLossless' || settings.mode === 'preset'" />
				<div class="grid grid-cols-2 gap-3">
					<label class="block">
						<span class="text-sm font-medium">Resize width</span>
						<input v-model.number="settings.resize.width" type="number" min="0" step="1" class="mt-1 w-full rounded border border-slate-300 bg-white p-1.5 text-sm dark:border-slate-700 dark:bg-slate-900">
					</label>
					<label class="block">
						<span class="text-sm font-medium">Resize height</span>
						<input v-model.number="settings.resize.height" type="number" min="0" step="1" class="mt-1 w-full rounded border border-slate-300 bg-white p-1.5 text-sm dark:border-slate-700 dark:bg-slate-900">
					</label>
				</div>
				<p class="text-xs text-slate-500 dark:text-slate-400">
					Resize is applied before encoding. Leave both at 0 for no resize; set one to 0
					to derive it from the other and keep the aspect ratio.
				</p>
			</ControlPanel>

			<details v-if="lossy" class="rounded-lg border border-slate-200 dark:border-slate-800" open>
				<summary class="cursor-pointer p-4 text-xs font-semibold tracking-wide text-slate-500 uppercase dark:text-slate-400">
					Advanced options
				</summary>
				<div class="flex flex-col gap-5 border-t border-slate-200 p-4 dark:border-slate-800">
					<div class="flex flex-col gap-4">
						<ControlSlider v-model="settings.sns" label="Spatial noise shaping" :min="0" :max="100" help="0 = off, 100 = maximum" />
						<ControlSlider v-model="settings.segments" label="Segments" :min="1" :max="4" help="number of segments to use (1..4)" />
						<ControlSlider v-model="settings.partitionLimit" label="Partition limit" :min="0" :max="100" help="quality degradation allowed to fit the 512k limit on prediction modes" />
						<ControlSlider v-model="settings.passes" label="Analysis passes" :min="1" :max="10" help="analysis pass number (1..10)" />
					</div>

					<div class="flex flex-col gap-3 border-t border-slate-200 pt-4 dark:border-slate-800">
						<span class="text-sm font-medium">Deblocking filter</span>
						<div class="flex flex-wrap gap-4">
							<label v-for="option in (['auto', 'simple', 'strong'] as const)" :key="option" class="flex items-center gap-1.5">
								<input v-model="settings.filter" type="radio" name="filter" :value="option" class="size-4 accent-sky-600">
								<span class="text-sm capitalize">{{ option }}</span>
							</label>
						</div>
						<p class="text-xs text-slate-500 dark:text-slate-400">
							Auto lets the encoder pick the strength, so the two sliders below do not apply.
						</p>
						<ControlSlider v-model="settings.filterStrength" label="Filter strength" :min="0" :max="100" help="0 = off .. 100 = strongest" :disabled="!manualFilter" />
						<ControlSlider v-model="settings.filterSharpness" label="Filter sharpness" :min="0" :max="7" help="0 = most sharp .. 7 = least sharp" :disabled="!manualFilter" />
					</div>

					<div class="flex flex-col gap-3 border-t border-slate-200 pt-4 dark:border-slate-800">
						<span class="text-sm font-medium">Compression target</span>
						<div class="flex flex-wrap gap-4">
							<label v-for="option in (['none', 'size', 'psnr'] as const)" :key="option" class="flex items-center gap-1.5">
								<input v-model="targetKind" type="radio" name="target" :value="option" class="size-4 accent-sky-600">
								<span class="text-sm">{{ option === 'none' ? 'Use quality' : option === 'size' ? 'Target file size' : 'Target PSNR' }}</span>
							</label>
						</div>
						<p class="text-xs text-slate-500 dark:text-slate-400">
							A size or PSNR target overrides the quality slider.
						</p>
						<label v-if="targetKind === 'size'" class="block">
							<span class="text-sm font-medium">Target size (bytes)</span>
							<input v-model.number="targetSize" type="number" min="1" step="1" class="mt-1 w-full rounded border border-slate-300 bg-white p-1.5 text-sm dark:border-slate-700 dark:bg-slate-900">
						</label>
						<ControlSlider v-if="targetKind === 'psnr'" v-model="targetPsnr" label="Target PSNR (dB)" :min="1" :max="10000" help="typically around 42" />
					</div>

					<div class="flex flex-col gap-3 border-t border-slate-200 pt-4 dark:border-slate-800">
						<ControlToggle v-model="settings.sharpYuv" label="Sharp YUV" help="use sharper (and slower) RGB to YUV conversion" />
						<ControlToggle v-model="settings.lowMemory" label="Low memory" help="reduce memory usage (slower encoding)" />
						<ControlToggle v-model="settings.multiThreading" label="Multi-threading" help="use multi-threading if available" />
					</div>
				</div>
			</details>

			<div class="flex flex-wrap items-center gap-4">
				<button
					type="button"
					class="rounded bg-sky-600 px-4 py-2 text-sm font-semibold text-white hover:bg-sky-700 disabled:cursor-not-allowed disabled:opacity-40"
					:disabled="!canConvert"
					@click="convert"
				>
					{{ busy ? 'Converting…' : 'Convert' }}
				</button>
				<p v-if="validationError" class="text-sm text-red-600 dark:text-red-400">
					{{ validationError }}
				</p>
			</div>

			<ControlPanel v-if="reports.length || failures.length" title="Results">
				<table v-if="reports.length" class="w-full text-left text-sm">
					<thead class="text-xs text-slate-500 uppercase dark:text-slate-400">
						<tr>
							<th class="py-1">File</th>
							<th class="py-1 text-right">Before</th>
							<th class="py-1 text-right">After</th>
							<th class="py-1 text-right">Change</th>
							<th class="py-1 text-right">Size</th>
						</tr>
					</thead>
					<tbody class="font-mono tabular-nums">
						<tr v-for="report in reports" :key="report.outputPath" class="border-t border-slate-200 dark:border-slate-800">
							<td class="py-1 pr-2 font-sans" data-selectable>{{ basename(report.outputPath) }}</td>
							<td class="py-1 text-right">{{ formatBytes(report.sourceBytes) }}</td>
							<td class="py-1 text-right">{{ formatBytes(report.outputBytes) }}</td>
							<td class="py-1 text-right" :class="report.outputBytes <= report.sourceBytes ? 'text-emerald-600 dark:text-emerald-400' : 'text-amber-600 dark:text-amber-400'">{{ saving(report) }}</td>
							<td class="py-1 text-right">{{ report.width }}&times;{{ report.height }}</td>
						</tr>
					</tbody>
				</table>
				<ul v-if="failures.length" class="flex flex-col gap-1 text-sm text-red-600 dark:text-red-400">
					<li v-for="failure in failures" :key="failure.path" data-selectable>
						<strong class="font-sans">{{ basename(failure.path) }}</strong>: {{ failure.message }}
					</li>
				</ul>
			</ControlPanel>
		</template>
	</main>
</template>
