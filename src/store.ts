/* eslint-disable prefer-const */
import { createStore } from 'vuex';
import * as path from 'path';
import { ipcRenderer } from 'electron';
import { State } from './interface/store/State';
import { CWEBPCommandParameters } from './interface/CWEBPCommandParameters';
import { Component } from './interface/store/Component';

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
                                        type: 'radio',
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
                                        type: 'select',
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
                                        type: 'slider',
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
                                        type: 'category',
                                        category: 'category',
                                        group: 'Mode',
                                        visible: false,
                                },
                                {
                                        type: 'category',
                                        category: 'category',
                                        group: 'CATquality',
                                        visible: false,
                                },
                                {
                                        type: 'category',
                                        category: 'category',
                                        group: 'CATcompression',
                                        visible: false,
                                },
                                {
                                        type: 'category',
                                        category: 'category',
                                        group: 'CATdeblocking',
                                        visible: false,
                                },
                                {
                                        type: 'category',
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
                                },
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
                        let radio = state.components.radio as Component[];
                        for (let i = 0; i < radio.length; i++) {
                                if (group === radio[i].group) {
                                        let val = radio[i].value as boolean[]
                                        for (let j = 0; j < val.length; j++) {
                                                let id = radio[i].id as string[]
                                                return (val[j] && id[j]) ?  id[j] : false;
                                        }
                                }
                        }
                },

                selectedSlider: (state) => (group: string) => { for (let i = 0; i < state.components.slider.length; i++) return state.components.slider[i].group === group ? state.components.slider[i] : false },

                selectedSelect: (state) => (group: string) => { for (let i = 0; i < state.components.select.length; i++) return state.components.select[i].group === group ? state.components.select[i] : false },
        },

        mutations: {
                setInputFiles: (state, payload: File) => state.inputFiles = payload,

                setOutputFiles: (state, payload: File) => state.outputFiles = payload,

                setAppIsDragEnter: (state, payload: boolean) => state.app.isDragEnter = payload,

                setAppCanDragEnter: (state, payload: boolean) => state.app.canDragEnter = payload,

                setAdvancedOptionsIsShown: (state, payload: boolean) => state.advancedOptions.isShown = payload,

                setAdvancedOptionsMode: (state, payload: string) => {
                        let mode = state.advancedOptions.mode;
                        // if key matches payload then mark it true else mark it false
                        for (const [key, value] of Object.entries(mode)) key === payload && typeof value === 'object' ? value.checked = true : value.checked = false;
                },

                setComponentValue: (state, payload: { type: Component['type']; group: Component['group']; value: Component['value'] }) => {
                        let existingComponents = state.components;
                        if(existingComponents[payload.type]) {
                                for(let i = 0; i < existingComponents[payload.type].length; i++) {
                                        if(existingComponents[payload.type][i].group === payload.group) {
                                                if(typeof payload.value === typeof state.components[payload.type][i].value) {
                                                        if(Array.isArray(payload.value) && Array.isArray(state.components[payload.type][i].value)) {
                                                                let componentValueArray = state.components[payload.type][i].value as boolean[];
                                                                if(payload.value.length === componentValueArray.length) state.components[payload.type][i].value = payload.value;
                                                        } else state.components[payload.type][i].value = payload.value;
                                                }
                                        }
                                }
                        }
                },

                registerComponent: (state, newComponent: Component) => {
                        //console.log('registerComponent: newComponent: ', JSON.stringify(newComponent, null, '\t'));
                        /* ************************************************ */
                        // begin housekeeping                               //
                        /* ************************************************ */
                        // remove and invaild components
                        // delete any groups that are undefined
                        for(const [typeKey] of Object.entries(state.components)) {
                                //console.log('registerComponent: typeKey: ', typeKey);
                                for(const [componentIndex, componentValue] of state.components[typeKey].entries()) {
                                        //console.log('registerComponent: componentIndex: ', componentIndex);
                                        //console.log('registerComponent: componentValue: ', JSON.stringify(componentValue,null,'\t'));
                                        if(componentValue.group === '') {
                                                delete state.components[typeKey][componentIndex];
                                                state.components[typeKey] = state.components[typeKey].filter( (a) => { return typeof a !== 'undefined' }) as Component[];
                                                //console.log('registerComponent: State: Components: ', JSON.stringify(state.components,null,'\t'));
                                        }
                                }
                        }

                        /* old */
                        /* for(let i = 0; i < Object.entries(state.components).length; i++) {
                                let j = 0;
                                console.log(JSON.stringify(state.components[Object.entries(state.components)[i][0]],null,'\t'));
                                state.components[Object.entries(state.components)[i][0]].forEach(() => {
                                        if(state.components[Object.entries(state.components)[i][0]][j].group === '') {
                                                delete state.components[Object.entries(state.components)[i][0]][j];
                                                state.components[Object.entries(state.components)[i][0]] = state.components[Object.entries(state.components)[i][0]].filter( (a) => { return typeof a !== 'undefined' });
                                        }
                                        j++;
                                });
                        } */
                        /* old */

                        /* ************************************************ */
                        // end housekeeping                                 //
                        /* ************************************************ */

                        //if component type exist
                        if(state.components[newComponent.type]) {
                                //console.log('Component type found.');

                                // if component type is empty push newComponent
                                if(state.components[newComponent.type].length === 0) state.components[newComponent.type].push(newComponent);

                                // if the component type already contains data
                                else {
                                        //console.log('Data existed on component type.');
                                        let matched = false;
                                        let i = 0;
                                        for(const component of state.components[newComponent.type]) {
                                                //console.log('registerComponent: component: ', JSON.stringify(component, null, '\t'));
                                                if(component) {
                                                        if(component.group === newComponent.group && component.category === newComponent.category) {
                                                                delete state.components[newComponent.type][i];
                                                                state.components[newComponent.type] = state.components[newComponent.type].filter((a) => { return typeof a !== 'undefined'});
                                                                state.components[newComponent.type].push(newComponent);
                                                                matched = true;
                                                        } else {
                                                                if(i === state.components[newComponent.type].length - 1 && !matched) state.components[newComponent.type].push(newComponent);
                                                        }
                                                }
                                                i++;
                                        }

                                        /* old */
                                        // for(let i = 0; i < state.components[newComponent.type].length; i++) {
                                        //         if(state.components[newComponent.type][i].group === newComponent.group && state.components[newComponent.type][i].category === newComponent.category) {
                                        //                 //console.log('Compnent group and category match found.');
                                        //                 delete state.components[newComponent.type][i];
                                        //                 state.components[newComponent.type] = state.components[newComponent.type].filter((a) => { return typeof a !== 'undefined'});
                                        //                 state.components[newComponent.type].push(newComponent);
                                        //                 matched = true;
                                        //         } else {
                                        //                 if(i === state.components[newComponent.type].length - 1 && !matched) {
                                        //                         //console.log('Compnent group and category match not found.');
                                        //                         state.components[newComponent.type].push(newComponent);
                                        //                 }
                                        //         }
                                        // }
                                        /* old */
                                }
                        } else state.components[newComponent.type].push(newComponent);

                        //console.log('Full Component State: ', JSON.stringify(state.components, null, '\t'));
                },

                /* registerComponentRadioGroup: (state, payload: { type: string; group: string; category: string;  id: string[]; value: boolean[]; association: string[]; visible: boolean }) => {
                        let radio = state.components.radio as { type: string; group: string; category: string;  id: string[]; value: boolean[]; association: string[]; visible: boolean }[];
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
                }, */

                /* registerComponentSelectGroup: (state, payload: { type: string; category: string; group: string; dependency: string; isOpen: boolean; label: string; value: string; visible: boolean }) => {
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
                }, */

                /* registerCompnentSliderGroup: (state, payload: { type: string; category: string; group: string; dependency: string; value: number; label: string; min: number; max: number; visible: boolean }) => {
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
                }, */
                
                deployAssociation: (state, payload: string) => {
                        //console.log('Payload: ', payload, ': Association Deployment Request Recieved.');
                        const assoc = state.advancedOptions.associations[payload as keyof {}] as { components: string[]; visible: boolean[] }
                        if (assoc) {
                                const associations = assoc.components;
                                const visible = state.advancedOptions.associations[payload as keyof {}]['visible'] as boolean[];
                                const components = state.components;
                                // if preset associations for payload group are found
                                if (associations) {
                                        //console.log('Original State: ', JSON.stringify(state,null,'\t'));
                                        // loop through all associations
                                        for(const [associationIndex, associationValue] of associations.entries()) {
                                                // loop through all components
                                                for(const [, componentValue] of Object.entries(components)) {
                                                        // loop through each item in each component
                                                        for(const val of componentValue) {
                                                                // if group matches association then set visible to true
                                                                if('visible' in val && val.group === associationValue) val.visible = visible[associationIndex];
                                                        }
                                                }
                                        }

                                        /* old */
                                        // for (let i = 0; i < associations.length; i++) {
                                        //         // loop through all components
                                        //         for (const [, componentValue] of Object.entries(components)) {
                                        //                 // loop through each item in each component
                                        //                 for (let j = 0; j < componentValue.length; j++) {
                                        //                         // if group matches association then set visible to true
                                        //                         if ('visible' in componentValue[j]) {
                                        //                                 if (componentValue[j].group == associations[i]) {
                                        //                                         console.log('Associative match found. Setting ' + componentValue[j].type + ' ' + componentValue[j].group + ' visibility to ' + visible[i]);
                                        //                                         componentValue[j].visible = visible[i];
                                        //                                         console.log('New State: ', JSON.stringify(state,null,'\t'));
                                        //                                 }
                                        //                         }
                                        //                 }
                                        //         }
                                        // }
                                        /* old */
                                        
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
                        if (promise) return promise;
                },

                setAdvancedOptionsIsShown({ getters, commit }, payload: boolean) {
                        const promise = new Promise((resolve, reject) => {
                                commit('setAdvancedOptionsIsShown', payload);
                                if (getters.advancedOptionsIsShown === payload) resolve(true);
                                else resolve(reject);
                        });

                        if (promise) return promise;
                },

                syncModeWithRadioGroup({ getters, commit }, payload: { group: string; id: string }) {
                        for(const radio of getters.componentsRadio) {
                                if(radio.group === payload.group) {
                                        for(const [idKey, idValue] of radio.id.entries()) {
                                                if(idValue === payload.id && radio.value[idKey]) {
                                                        commit('setAdvancedOptionsMode', payload.id);
                                                }
                                        }
                                }
                        }

                        /* for (let i = 0; i < radios.length; i++) {
                                if (radios[i].group === payload.group) {
                                        for (let j = 0; j < radios[i].id.length; j++) {
                                                if (radios[i].id[j] === payload.id && radios[i].value[j] === true) {
                                                        commit('setAdvancedOptionsMode', payload.id);
                                                }
                                        }
                                }
                        } */
                },

                constructCMD({ getters }) {
                        const inputFiles = (): string => { return getters.inputFiles[0] ? getters.inputFiles[0] : 'undefined' };
                        const ouputFiles = (): string => { return getters.outputFiles[0] ? getters.outputFiles[0] : 'undefined' };
                        const mode = (): string => { return getters.selectedRadio('Mode') ? getters.selectedRadio('Mode') : 'undefined' };
                        const isAdvanced = (): boolean => { return mode() !== 'undefined' ? true : false };
                        const preset = (): string => { return getters.selectedSelect('preset').value === '' ? 'undefined' : getters.selectedSelect('preset').value };
                        const quality = (): string => { return getters.selectedSlider('quality').value ? getters.selectedSlider('quality').value : 'undefined' };
                        const alphaQuality = (): string => { return getters.selectedSlider('alphaQuality').value ? getters.selectedSlider('alphaQuality').value : 'undefined' };
                        const compressionMethod = (): string => { return getters.selectedSlider('compressionMethod').value ? getters.selectedSlider('compressionMethod').value : 'undefined' };
                        const compression = (): string => { return getters.selectedRadio('compression') ? getters.selectedRadio('compression') : 'undefined' };
                        const compressionTarget = (): string => { return compression() === 'fileSize' ? getters.selectedSlider('targetSize').value : compression() == 'PSNR' ? getters.selectedSlider('targetPSNR').value : 'undefined' };
                        const numberOfPasses = (): string => { return getters.selectedSlider('numberOfPasses').value ? getters.selectedSlider('numberOfPasses').value : 'undefined' };
                        const deblockingFilter = (): string => { return getters.selectedRadio('deblocking') ? getters.selectedRadio('deblocking') : 'undefined' };
                        const deblockingStrength = (): string => { return deblockingFilter() === 'undefined' || deblockingFilter() === 'autoFilter' ? 'undefined' : getters.selectedSlider('strength').value };
                        const deblockingSharpness = (): string => { return deblockingFilter() === 'undefined' || deblockingFilter() === 'autoFilter' ? 'undefined' : getters.selectedSlider('sharpness').value };
                        const noiseShapingAmplitude = (): string => { return getters.selectedSlider('amplitude').value ? getters.selectedSlider('amplitude').value : 'undefined' };
                        const noiseShapingSegments = (): string => { return getters.selectedSlider('numberOfSegments').value ? getters.selectedSlider('numberOfSegments').value : 'undefined' };
                        const binariesPath = (): string => { 
                                let outputPath = path.join("./resources", "cwebp");
                                const isDev = ipcRenderer.sendSync('isDev');
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

                                if (platform() && isDev) outputPath = path.join(rootPath, "./resources", platform(), "cwebp");
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

                        //console.log(JSON.stringify(CMDObject, null, '\t'));

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

                                        const qualityFlag = (): string => { return payload.quality !== 'undefinded' ? ' -q ' + payload.quality : '' };
                                        const alphaQualityFlag = (): string => { return payload.alphaQuality !== 'undefinded' ? ' -alpha_q ' + payload.alphaQuality : '' };
                                        const compressionMethodFlag = (): string => { return payload.alphaQuality !== 'undefined' ? ' -m ' + payload.compressionMethod : '' };
                                        const multiThreadingFlag = (): string => { return ' -mt' };

                                        const compressionFlag = (): string => {
                                                switch (payload.compression) {
                                                case 'fileSize':
                                                        return  ' -size ' + payload.compressionTarget;

                                                case 'PSNR':
                                                        return ' -psnr ' + payload.compressionTarget;
                                        
                                                default:
                                                        return '';
                                                }
                                        };

                                        const numberOfPassesFlag = (): string => { return payload.numberOfPasses !== 'undefined' ? ' -pass ' + payload.numberOfPasses : '' };
                                        const deblockingStrengthFlag = (): string => { return payload.deblockingStrength !== 'undefined' ? ' -f ' + payload.deblockingStrength : '' };

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
                                        };

                                        const deblockingSharpnessFlag = (): string => { return payload.deblockingSharpness !== 'undefined' ? ' -sharpness ' + payload.deblockingSharpness : '' };
                                        const sharpYUVFlag = (): string => { return ' -sharp_yuv' };
                                        const noiseShapingAmplitudeFlag = (): string => { return payload.noiseShapingAmplitude !== 'undefined' ? ' -sns ' + payload.noiseShapingAmplitude : '' };
                                        const noiseShapingSegmentsFlag = (): string => { return payload.noiseShapingSegments !== 'undefined' ? ' -segments ' + payload.noiseShapingSegments : '' };

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
