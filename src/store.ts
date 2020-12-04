/* eslint-disable prefer-const */
import { createStore } from 'vuex';
import * as path from 'path';
import { ipcRenderer } from 'electron';
import { State } from './interface/store/State'
import { CWEBPCommandParameters } from './interface/CWEBPCommandParameters'

export const store = createStore({
        state: {
                inputFiles: {},
                outputFiles: {},
                app: {
                        isDragEnter: false,
                        canDragEnter: true,
                },
                components: {
                        radio: [
                                {
                                        category: '',
                                        group: '',
                                        id: [''],
                                        association: [''],
                                        value: [false],
                                        visible: false,
                                }
                        ],
                        select: [
                                {
                                        category: '',
                                        group: '',
                                        dependency: '',
                                        label: '',
                                        value: '',
                                        isOpen: false,
                                        visible: false,
                                }
                        ],
                        slider: [
                                {

                                        category: '',
                                        group: '',
                                        dependency: '',
                                        label: '',
                                        value: 1,
                                        valueMin: 1,
                                        valueMax: 1,
                                        visible: false,
                                }
                        ],
                        category: [
                                {
                                        category: 'category',
                                        group: 'CATquality',
                                        visible: false,
                                },
                                {
                                        category: 'category',
                                        group: 'CATcompression',
                                        visible: false,
                                },
                                {
                                        category: 'category',
                                        group: 'CATdeblocking',
                                        visible: false,
                                },
                                {
                                        category: 'category',
                                        group: 'CATnoiseShaping',
                                        visible: false,
                                },
                        ]
                },
                advancedOptions: {
                        isShown: false,
                        associations: {
                                jpegLike: {
                                        components: ['numberOfSegments', 'amplitude', 'CATnoiseShaping', 'CATquality', 'CATcompression', 'CATdeblocking', 'preset', 'quality', 'alphaQuality', 'compressionMethod', 'numberOfPasses'],
                                        visible: [false, false, false, true, false, false, false, true, true, true, false],
                                },
                                lossy: {
                                        components: ['numberOfSegments', 'amplitude', 'CATnoiseShaping', 'CATquality', 'CATcompression', 'CATdeblocking', 'preset', 'quality', 'alphaQuality', 'compressionMethod', 'numberOfPasses'],
                                        visible: [true, true, true, true, true, true, false, true, true, true, true],
                                },
                                lossless: {
                                        components: ['numberOfSegments', 'amplitude', 'CATnoiseShaping', 'CATquality', 'CATcompression', 'CATdeblocking', 'preset', 'quality', 'alphaQuality', 'compressionMethod', 'numberOfPasses'],
                                        visible: [false, false, false, true, false, false, false, true, true, true, false],
                                },
                                nearLossless: {
                                        components: ['numberOfSegments', 'amplitude', 'CATnoiseShaping', 'CATquality', 'CATcompression', 'CATdeblocking', 'preset', 'quality', 'alphaQuality', 'compressionMethod', 'numberOfPasses'],
                                        visible: [false, false, false, true, false, false, false, true, false, false, false],
                                },
                                preset: {
                                        components: ['numberOfSegments', 'amplitude', 'CATnoiseShaping', 'CATquality', 'CATcompression', 'CATdeblocking', 'preset', 'quality', 'alphaQuality', 'compressionMethod', 'numberOfPasses'],
                                        visible: [false, false, false, true, false, false, true, true, false, false, false],
                                },
                                fileSize: {
                                        components: ['targetSize', 'targetPSNR'],
                                        visible: [true, false],
                                },
                                PSNR: {
                                        components: ['targetSize', 'targetPSNR'],
                                        visible: [false, true],
                                },
                                autoFilter: {
                                        components: ['strength', 'sharpness'],
                                        visible: [false, false],
                                },
                                strongFilter: {
                                        components: ['strength', 'sharpness'],
                                        visible: [true, true],
                                },
                                simpleFilter: {
                                        components: ['strength', 'sharpness'],
                                        visible: [true, true],
                                }
                        },
                        mode: {
                                jpegLike: {
                                        checked: false,
                                        label: 'Change the internal parameter mapping to better match the expected size of JPEG compression. This flag will generally produce an output file of similar size to its JPEG equivalent (for the same quality setting), but with less visual distortion.',
                                },
                                lossy: {
                                        checked: false,
                                        label: 'These options are only effective when doing lossy encoding (the default, with or without alpha).',
                                },
                                lossless: {
                                        checked: false,
                                        label: 'Display options that are available when doing lossless encoding.',
                                },
                                nearLossless: {
                                        checked: false,
                                        label: 'Specify the level of near-lossless image preprocessing. This option adjusts pixel values to help compressibility, but has minimal impact on the visual quality. It triggers lossless compression mode automatically. The range is 0 (maximum preprocessing) to 100 (no preprocessing, the default). The typical value is around 60. Note that lossy with quality 100 can at times yield better results.',
                                },
                                preset: {
                                        checked: false,
                                        label: 'Specify a set of pre-defined parameters to suit a particular type of source material. Possible values are: default, photo, picture, drawing, icon, text. Since preset overwrites the other parameters values (except the quality one), this option should preferably appear first in the order of the arguments.',
                                },
                        },
                        quality: {
                                preset: [
                                        {
                                                group: 'preset',
                                                label: 'Select a Preset',
                                                value: '',
                                        },
                                        {
                                                group: 'preset',
                                                label: 'Default',
                                                value: 'default',
                                        },
                                        {
                                                group: 'preset',
                                                label: 'Photo',
                                                value: 'photo',
                                        },
                                        {
                                                group: 'preset',
                                                label: 'Picture',
                                                value: 'picture',
                                        },
                                        {
                                                group: 'preset',
                                                label: 'Drawing',
                                                value: 'drawing',
                                        },
                                        {
                                                group: 'preset',
                                                label: 'Icon',
                                                value: 'icon',
                                        },
                                        {
                                                group: 'preset',
                                                label: 'Text',
                                                value: 'text',
                                        },
                                ],
                                quality: {
                                        group: 'quality',
                                        label: 'Quality',
                                        value: 100,
                                        min: 1,
                                        max: 100,
                                },
                                alphaquality: {
                                        group: 'alphaQuality',
                                        label: 'Alpha Quality',
                                        value: 100,
                                        min: 1,
                                        max: 100,
                                },
                                compressionmethod: {
                                        group: 'compressionMethod',
                                        label: 'Compression Method',
                                        value: 4,
                                        min: 0,
                                        max: 6,
                                }
                        },
                        compression: {
                                fileSize: {
                                        checked: true,
                                        label: 'Specify a target size (in bytes) to try and reach for the compressed output. The compressor will make several passes of partial encoding in order to get as close as possible to this target. If both -size and -psnr are used, -size value will prevail.',
                                },
                                PSNR: {
                                        checked: false,
                                        label: 'Specify a target PSNR (in dB) to try and reach for the compressed output. The compressor will make several passes of partial encoding in order to get as close as possible to this target. If both -size and -psnr are used, -size value will prevail.',
                                },
                                targetsize: {
                                        group: 'targetSize',
                                        label: 'Target Size (bytes)',
                                        value: 4,
                                        min: 1,
                                        max: 10000,
                                },
                                targetpsnr: {
                                        group: 'targetPSNR',
                                        label: 'Target PSNR (decibels)',
                                        value: 4,
                                        min: 1,
                                        max: 10000,
                                },
                                numberofpasses: {
                                        group: 'numberOfPasses',
                                        label: 'Number of Passes',
                                        value: 6,
                                        min: 1,
                                        max: 10,
                                }
                        },
                        deblocking: {
                                autoFilter: {
                                        checked: true,
                                        label: 'Turns auto-filter on. This algorithm will spend additional time optimizing the filtering strength to reach a well-balanced quality.',
                                },
                                strongFilter: {
                                        checked: true,
                                        label: 'Use strong filtering (if filtering is being used thanks to the -f option). Strong filtering is on by default.',
                                },
                                simpleFilter: {
                                        checked: true,
                                        label: 'Disable strong filtering (if filtering is being used thanks to the -f option) and use simple filtering instead.',
                                },
                                strength: {
                                        group: 'strength',
                                        label: 'Strength',
                                        value: 20,
                                        min: 0,
                                        max: 100,
                                },
                                sharpness: {
                                        group: 'sharpness',
                                        label: 'Sharpness',
                                        value: 20,
                                        min: 0,
                                        max: 7,
                                },
                        },
                        noiseshaping: {
                                amplitude: {
                                        group: 'amplitude',
                                        label: 'amplitude',
                                        value: 50,
                                        min: 0,
                                        max: 100,
                                },
                                numberofsegments: {
                                        group: 'numberOfSegments',
                                        label: 'Number of Segments',
                                        value: 4,
                                        min: 1,
                                        max: 4,
                                },
                        },
                },
        } as State,

        getters: {
                inputFiles: state => { return state.inputFiles },
                outputFiles: state => { return state.outputFiles },
                appIsDragEnter: state => { return state.app.isDragEnter },
                appCanDragEnter: state => { return state.app.canDragEnter },
                componentsRadio: state => { return state.components.radio },
                componentsSelect: state => { return state.components.select },
                componentSlider: state => { return state.components.slider },
                componentCategory: state => { return state.components.category },
                advancedOptions: state => { return state.advancedOptions },
                advancedOptionsIsShown: state => { return state.advancedOptions.isShown },
                selectedRadio: (state) => (group: string) => { 
                        let radio = state.components.radio as { group: string; category: string;  id: string[]; value: boolean[]; association: string[]; visible: boolean }[];
                        for (let i = 0; i < radio.length; i++) {
                                if (group === radio[i].group) {
                                        for (let j = 0; j < radio[i].value.length; j++) {
                                                if (radio[i].value[j] === true) {
                                                        return radio[i].id[j];
                                                }
                                        }
                                }
                        }
                },
                selectedSlider: (state) => (group: string) => { for (let i = 0; i < state.components.slider.length; i++) if (state.components.slider[i].group === group) return state.components.slider[i] },
                selectedSelect: (state) => (group: string) => { for (let i = 0; i < state.components.select.length; i++) if (state.components.select[i].group === group) return state.components.select[i] },
        },

        mutations: {
                setInputFiles: (state, payload: File) => state.inputFiles = payload,
                setOutputFiles: (state, payload: File) => state.outputFiles = payload,
                setAppIsDragEnter: (state, payload: boolean) => state.app.isDragEnter = payload,
                setAppCanDragEnter: (state, payload: boolean) => state.app.canDragEnter = payload,
                setAdvancedOptionsIsShown: (state, payload: boolean) => state.advancedOptions.isShown = payload,
                setAdvancedOptionsMode: (state, payload: string) => {
                        let mode = state.advancedOptions.mode;
                        // if key matches payload the mark it true else mark it false
                        for (const [key, value] of Object.entries(mode)) {
                                if (typeof value === 'object') {
                                        if (key === payload) value.checked = true;
                                        else value.checked = false;
                                }
                        }
                },
                registerComponentRadioGroup: (state, payload: { group: string; category: string;  id: string[]; value: boolean[]; association: string[]; visible: boolean }) => {
                        let radio = state.components.radio as { group: string; category: string;  id: string[]; value: boolean[]; association: string[]; visible: boolean }[];
                        let mygroup = payload;

                        // set existing buttons clicked to false
                        for (let i = 0; i < radio.length; i++) {
                                for (let j = 0; j < (radio[i].value.length); j++) {
                                        if(radio[i].group === mygroup.group) {
                                                radio[i].value[j] = false;
                                        }
                                }
                        }

                        for (let i = 0; i < radio.length; i++) {
                                // merge any prevous groups with the same name
                                if (radio[i].group === mygroup.group) {
                                        // delete matching IDs and corresponding checked
                                        for (let j = 0; j < radio[i].id.length; j++) {
                                                if (radio[i].id[j] === mygroup.id[0]) {
                                                        // delete
                                                        delete radio[i].id[j];
                                                        delete radio[i].value[j];

                                                        // purge undefined
                                                        radio[i].id = radio[i].id.filter((a) => { return typeof a !== 'undefined' });
                                                        radio[i].value = radio[i].value.filter((a) => { return typeof a !== 'undefined' });

                                                }
                                        }

                                        // merge group ids
                                        radio[i].id.push(mygroup.id[0]);
                                        mygroup.id = radio[i].id;

                                        // merge group checked
                                        radio[i].value.push(mygroup.value[0]);
                                        mygroup.value = radio[i].value;

                                        // delete matching group from radio
                                        delete radio[i]; radio = radio.filter((a) => { return typeof a !== 'undefined' });
                                }
                        }

                        // delete any groups that are undefined
                        let i = 0;
                        state.components.radio.forEach((group) => {
                                if (group.group === '') {
                                        delete radio[i]; radio = radio.filter((a) => { return typeof a !== 'undefined' });
                                }
                                i++;
                        });

                        radio.push(mygroup);
                        state.components.radio = radio;
                },
                registerComponentSelectGroup: (state, payload: { category: string; group: string; dependency: string; isOpen: boolean; label: string; value: string; visible: boolean }) => {
                        let select = state.components.select;

                        // delete any existing items with the same group or where group is empty
                        for (let i = 0; i < select.length; i++) {
                                if (select[i].group === payload.group) {
                                        delete select[i];
                                        select = select.filter((a) => { return typeof a !== 'undefined' });
                                } else if (select[i].group === '') {
                                        delete select[i]; select = select.filter((a) => { return typeof a !== 'undefined' });
                                }
                        }

                        select.push(payload);
                        state.components.select = select;
                },
                registerCompnentSliderGroup: (state, payload: { category: string; group: string; dependency: string; value: number; label: string; min: number; max: number; visible: boolean }) => {
                        let slider = state.components.slider;

                        // delete any existing items with the same name or where name is empty
                        for (let i = 0; i < slider.length; i++) {
                                if (slider[i].group === payload.group) {
                                        delete slider[i];
                                        slider = slider.filter((a) => { return typeof a !== 'undefined' });
                                } else if (slider[i].group === '') {
                                        delete slider[i]; slider = slider.filter((a) => { return typeof a !== 'undefined' });
                                }
                        }

                        slider.push(payload);
                        state.components.slider = slider;

                        //console.log('slider: ', slider);
                },
                deployAssociation: (state, payload: string) => {
                        const assoc = state.advancedOptions.associations[payload as keyof {}] as { components: string[]; visible: boolean[] }
                        if (assoc) {
                                const associations = assoc.components;
                                const visible = state.advancedOptions.associations[payload as keyof {}]['visible'] as boolean[];
                                const components = state.components;
                                // if preset associations for payload group are found
                                if (associations) {
                                        // loop through all associations
                                        for (let i = 0; i < associations.length; i++) {
                                                // loop through all components
                                                for (const [, componentValue] of Object.entries(components)) {
                                                        // loop through each item in each component
                                                        for (let j = 0; j < componentValue.length; j++) {
                                                                // if group matches association then set visible to true
                                                                if ('visible' in componentValue[j]) {
                                                                        if (componentValue[j].group == associations[i]) {
                                                                                componentValue[j].visible = visible[i];
                                                                                //console.log(componentValue[j].group, ' Visibility: ', componentValue[j].visible);
                                                                        }
                                                                }
                                                        }
                                                }
                                        }
                                }
                        }
                }
        },

        actions: {

                addFiles({ commit }, payload: { type: string; paths: string[] }) {
                        switch(payload.type) {
                        case "output":
                                return new Promise(() => commit('setOutputFiles', payload.paths));
                        default:
                                return new Promise(() => commit('setInputFiles', payload.paths));
                        }
                },
                setAppIsDragEnter({ getters, commit }, payload: boolean) {
                        const promise = new Promise((resolve, reject) => {
                                commit('setAppIsDragEnter', payload);
                                if (getters.appIsDragEnter === payload) resolve(true)
                                else resolve(reject);
                        });
                        if (promise) return promise; //console.log('Store :: setAppIsDragEnter: ', payload);
                },
                setAdvancedOptionsIsShown({ getters, commit }, payload: boolean) {
                        const promise = new Promise((resolve, reject) => {
                                commit('setAdvancedOptionsIsShown', payload);
                                if (getters.advancedOptionsIsShown === payload) resolve(true);
                                else resolve(reject);
                        });

                        if (promise) return promise; //console.log('Store :: setAdvancedOptionsIsShown: ', payload);
                },
                syncModeWithRadioGroup({ getters, commit }, payload: { group: string; id: string }) {
                        const radios = getters.componentsRadio;
                        for (let i = 0; i < radios.length; i++) {
                                if (radios[i].group === payload.group) {
                                        for (let j = 0; j < radios[i].id.length; j++) {
                                                if (radios[i].id[j] === payload.id && radios[i].value[j] === true) {
                                                        commit('setAdvancedOptionsMode', payload.id);
                                                }
                                        }
                                }
                        }
                },
                constructCMD({ getters }) {
                        const inputFiles = (): string => {
                                if (getters.inputFiles[0]) return getters.inputFiles[0];
                                else return 'undefined'
                        };
                        //console.log('Input File: ', inputFiles());

                        const ouputFiles = (): string => {
                                if (getters.outputFiles[0]) return getters.outputFiles[0];
                                else return 'undefined';
                        };
                        //console.log('Output File: ', ouputFiles());

                        const mode = (): string => {
                                if (getters.selectedRadio('Mode')) return getters.selectedRadio('Mode');
                                else return 'undefined';
                        };
                        //console.log('Mode: ', mode());

                        const isAdvanced = (): boolean => {
                                if(mode() !== 'undefined') return true;
                                else return false;
                        }

                        const preset = (): string => {
                                if (getters.selectedSelect('preset').value === '') return 'undefined';
                                else return getters.selectedSelect('preset').value;
                        }
                        //console.log('Preset: ', preset());

                        const quality = (): string => {
                                if (getters.selectedSlider('quality').value) return getters.selectedSlider('quality').value;
                                else return 'undefined';
                        };
                        //console.log('Quality: ', quality());

                        const alphaQuality = (): string => {
                                if (getters.selectedSlider('alphaQuality').value) return getters.selectedSlider('alphaQuality').value;
                                else return 'undefined';
                        };
                        //console.log('Alpha Quality: ', alphaQuality());

                        const compressionMethod = (): string => {
                                if (getters.selectedSlider('compressionMethod').value) return getters.selectedSlider('compressionMethod').value;
                                else return 'undefined';
                        };
                        //console.log('Compression Method: ', compressionMethod());

                        const compression = (): string => {
                                if (getters.selectedRadio('compression')) return getters.selectedRadio('compression');
                                else return 'undefined';
                        };
                        //console.log('Compression: ', compression());

                        const compressionTarget = (): string => {
                                if (compression() === 'fileSize') return getters.selectedSlider('targetSize').value;
                                else if (compression() == 'PSNR') return getters.selectedSlider('targetPSNR').value;
                                else return 'undefined';
                        };
                        //console.log('Compression Taget: ', compressionTarget());

                        const numberOfPasses = (): string => {
                                if (getters.selectedSlider('numberOfPasses').value) return getters.selectedSlider('numberOfPasses').value;
                                else return 'undefined';
                        };
                        //console.log('Number of Passes: ', numberOfPasses());

                        const deblockingFilter = (): string => {
                                if (getters.selectedRadio('deblocking')) return getters.selectedRadio('deblocking');
                                else return 'undefined';
                        };
                        //console.log('Deblocking Filter: ', deblockingFilter());

                        const deblockingStrength = (): string => {
                                if (deblockingFilter() === 'undefined' || deblockingFilter() === 'autoFilter') return 'undefined';
                                else return getters.selectedSlider('strength').value;
                        };
                        //console.log('Deblocking Filter Strength: ', deblockingStrength());

                        const deblockingSharpness = (): string => {
                                if (deblockingFilter() === 'undefined' || deblockingFilter() === 'autoFilter') return 'undefined';
                                else return getters.selectedSlider('sharpness').value;
                        };
                        //console.log('Deblocking Filter Sharpness: ', deblockingSharpness());

                        const noiseShapingAmplitude = (): string => {
                                if (getters.selectedSlider('amplitude').value) return getters.selectedSlider('amplitude').value;
                                else return 'undefined';
                        };
                        //console.log('Noise Shaping Amplitude: ', noiseShapingAmplitude());

                        const noiseShapingSegments = (): string => {
                                if (getters.selectedSlider('numberOfSegments').value) return getters.selectedSlider('numberOfSegments').value;
                                else return 'undefined';
                        };
                        //console.log('Number of Noise Shaping Segments: ', noiseShapingSegments());

                        const binariesPath = (): string => {
                                let outputPath = path.join("./resources", "cwebp");
                                const isDev = ipcRenderer.sendSync('isDev');
                                //console.log('isDev: ', isDev);
                                const rootPath = process.cwd();
                                const platform = (): string => {
                                        let plat = 'undefined';
                                        switch (process.platform) {
                                        case 'aix': case 'freebsd': case 'linux': case 'openbsd': case 'android': plat = 'linux'; break;
                                        case 'darwin': case 'sunos': plat = 'mac'; break;
                                        case 'win32': plat = 'win'; break;
                                        }
                                        return plat;
                                }

                                if (platform() && isDev) {
                                        //console.log('Running in Development Mode');
                                        outputPath = path.join(rootPath, "./resources", platform(), "cwebp");
                                } else {
                                        //console.log('Running in Production Mode');
                                }

                                outputPath = path.resolve(path.join(outputPath, "./cwebp"));

                                return outputPath;
                        }

                        const CMDObject: CWEBPCommandParameters =
                                {
                                        binariesPath: binariesPath(),
                                        inputFiles: inputFiles(),
                                        ouputFiles: ouputFiles(),
                                        isAdvanced: isAdvanced(),
                                        mode: mode(),
                                        preset: preset(),
                                        quality: quality(),
                                        alphaQuality: alphaQuality(),
                                        compressionMethod: compressionMethod(),
                                        compression: compression(),
                                        compressionTarget: compressionTarget(),
                                        numberOfPasses: numberOfPasses(),
                                        deblockingFilter: deblockingFilter(),
                                        deblockingStrength: deblockingStrength(),
                                        deblockingSharpness: deblockingSharpness(),
                                        noiseShapingAmplitude: noiseShapingAmplitude(),
                                        noiseShapingSegments: noiseShapingSegments(),
                                }
                        console.log(JSON.stringify(CMDObject, null, '\t'));

                        const command = (payload: CWEBPCommandParameters): string => {

                                let CMD = '';

                                if(payload.isAdvanced) {

                                        const modeFlag = (): string => {
                                                switch (payload.mode) {
                                                case 'lossless':
                                                        return ' -lossless -exact';
        
                                                case 'jpegLike':
                                                        return ' -jpeg_like';
        
                                                case 'nearLossless':
                                                        return ' -near_lossless' + payload.quality;
                                                
                                                case 'preset':
                                                        return ' -preset ' + payload.preset;
                                                
                                                default:
                                                        return '';
                                                }
                                        }
                                        console.log('modeFlag: ', modeFlag());

                                        const qualityFlag = (): string => {
                                                if(payload.quality !== 'undefinded') return ' -q ' + payload.quality;
                                                else return '';
                                        }
                                        console.log('qualityFlag: ', qualityFlag());

                                        const alphaQualityFlag = (): string => {
                                                if(payload.alphaQuality !== 'undefinded') return ' -alpha_q ' + payload.alphaQuality
                                                else return '';
                                        }
                                        console.log('alphaQualityFlag: ', alphaQualityFlag());

                                        const compressionMethodFlag = (): string => {
                                                if(payload.alphaQuality !== 'undefined') return ' -m ' + payload.compressionMethod
                                                else return '';
                                        }
                                        console.log('compressionMethodFlag: ', compressionMethodFlag());

                                        const multiThreadingFlag = (): string => {
                                                return ' -mt';
                                        }
                                        console.log('multiThreadingFlag: ', multiThreadingFlag());

                                        const compressionFlag = (): string => {
                                                switch (payload.compression) {
                                                case 'fileSize':
                                                        return  ' -size ' + payload.compressionTarget;

                                                case 'PSNR':
                                                        return ' -psnr ' + payload.compressionTarget;
                                        
                                                default:
                                                        return '';
                                                }
                                        }
                                        console.log('compressionFlag: ', compressionFlag());

                                        const numberOfPassesFlag = (): string => {
                                                if(payload.numberOfPasses !== 'undefined') return ' -pass ' + payload.numberOfPasses;
                                                else return '';
                                        }
                                        console.log('numberOfPassesFlag: ', numberOfPassesFlag());

                                        const deblockingStrengthFlag = (): string => {
                                                if(payload.deblockingStrength !== 'undefined') return ' -f ' + payload.deblockingStrength;
                                                else return '';
                                        }
                                        console.log('deblockingStrengthFlag: ', deblockingStrengthFlag());

                                        const deblockingFilterFlag = (): string => {
                                                switch (payload.deblockingFilter) {
                                                case 'strongFilter':
                                                        return ' -strong';
                                                case 'simpleFilter':
                                                        return ' -nostrong';
                                                case 'autoFilter':
                                                        return ' -af'
                                                default:
                                                        return '';
                                                }
                                        }
                                        console.log('deblockingFilterFlag: ', deblockingFilterFlag());

                                        const deblockingSharpnessFlag = (): string => {
                                                if(payload.deblockingSharpness !== 'undefined') return ' -sharpness ' + payload.deblockingSharpness;
                                                else return '';
                                        }
                                        console.log('deblockingSharpnessFlag: ', deblockingSharpnessFlag());

                                        const sharpYUVFlag = (): string => {
                                                return ' -sharp_yuv';
                                        }
                                        console.log('sharpYUVFlag: ', sharpYUVFlag());

                                        const noiseShapingAmplitudeFlag = (): string => {
                                                if(payload.noiseShapingAmplitude !== 'undefined') return ' -sns ' + payload.noiseShapingAmplitude;
                                                else return '';
                                        }
                                        console.log('noiseShapingAmplitudeFlag: ', noiseShapingAmplitudeFlag());

                                        const noiseShapingSegmentsFlag = (): string => {
                                                if(payload.noiseShapingSegments !== 'undefined') return ' -segments ' + payload.noiseShapingSegments;
                                                else return '';
                                        }
                                        console.log('noiseShapingSegmentsFlag: ', noiseShapingSegmentsFlag());

                                        CMD = payload.binariesPath + modeFlag() + qualityFlag() + alphaQualityFlag() + compressionMethodFlag() + multiThreadingFlag() + compressionFlag() + numberOfPassesFlag() + deblockingStrengthFlag() + deblockingFilterFlag() + deblockingSharpnessFlag() + sharpYUVFlag() + noiseShapingAmplitudeFlag() + noiseShapingSegmentsFlag();
                                        console.log(CMD);

                                }
                                
                                return JSON.stringify(CMD, null, '\t');
                        }

                        return command(CMDObject);

                },
        }
});

export default store;
