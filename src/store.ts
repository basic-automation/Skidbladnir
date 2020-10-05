/* eslint-disable prefer-const */
import { createStore } from 'vuex';

export const store = createStore({
        state: {
                inputFiles: {},
                outputFiles: {},
                app: {
                        isDragEnter: false,
                        canDragEnter: true,
                },
                components: {
                        radio:
                        [
                                {
                                        group: '',
                                        id: [''],
                                        checked: [false],
                                        association: [''],
                                        visible: false,
                                }
                        ],
                        select:
                        [
                                {
                                        category: '',
                                        group: '',
                                        dependency: '',
                                        isOpen: false,
                                        label: '',
                                        value: '',
                                        visible: false,
                                }
                        ],
                },
                advancedOptions: {
                        isShown: false,
                        associations: 
                        {
                                jpegLike: {
                                        components: ['preset'],
                                        visible: [false]
                                },
                                lossy: {
                                        components: ['preset'],
                                        visible: [false]
                                },
                                lossless: {
                                        components: ['preset'],
                                        visible: [false]
                                },
                                nearLossless: {
                                        components: ['preset'],
                                        visible: [false]
                                },
                                preset: {
                                        components: ['preset'],
                                        visible: [true]
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
                                preset: 
                                [
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
                                
                        }
                },
        },

        getters: {
                inputFiles: state => { return state.inputFiles },
                outputFiles: state => { return state.outputFiles },
                appIsDragEnter: state => { return state.app.isDragEnter },
                appCanDragEnter: state => { return state.app.canDragEnter },
                componentsRadio: state => { return state.components.radio },
                componentsSelect: state => { return state.components.select },
                advancedOptions: state => { return state.advancedOptions },
                advancedOptionsIsShown: state => { return state.advancedOptions.isShown },
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
                        for(const [key, value] of Object.entries(mode)) {
                                if(typeof value === 'object') {
                                        if(key === payload) value.checked = true;
                                        else value.checked = false;
                                }
                        }
                },
                registerComponentRadioGroup: (state, payload: { group: string; id: string[]; checked: boolean[]; association: string[]; visible: boolean }) => {
                        let radio = state.components.radio;
                        let mygroup = payload;

                        // set existing buttons clicked to false
                        for(let i = 0; i < radio.length; i++) {
                                for(let j = 0; j < (radio[i].checked.length); j++) {
                                        radio[i].checked[j] = false;
                                }
                        }

                        for(let i = 0; i < radio.length; i++) {
                                // merge any prevous groups with the same name
                                if(radio[i].group === mygroup.group) {
                                        // delete matching IDs and corresponding checked
                                        for(let j = 0; j < radio[i].id.length; j++) {
                                                if(radio[i].id[j] === mygroup.id[0]) {
                                                        // delete
                                                        delete radio[i].id[j];
                                                        delete radio[i].checked[j];

                                                        // purge undefined
                                                        radio[i].id = radio[i].id.filter((a) => { return typeof a !== 'undefined'});
                                                        radio[i].checked = radio[i].checked.filter((a) => { return typeof a !== 'undefined'});

                                                }
                                        }
                                        
                                        // merge group ids
                                        radio[i].id.push(mygroup.id[0]);
                                        mygroup.id = radio[i].id;

                                        // merge group checked
                                        radio[i].checked.push(mygroup.checked[0]);
                                        mygroup.checked = radio[i].checked;

                                        // delete matching group from radio
                                        delete radio[i]; radio = radio.filter((a) => { return typeof a !== 'undefined'});
                                }
                        }

                        // delete any groups that are undefined
                        let i = 0;
                        state.components.radio.forEach((group) => {
                                if(group.group === '') {
                                        delete radio[i]; radio = radio.filter((a) => { return typeof a !== 'undefined'});
                                }
                                i++;
                        });

                        radio.push(mygroup);
                        state.components.radio = radio;
                },
                registerComponentSelectGroup: (state, payload: { category: string; group: string; dependency: string; isOpen: boolean; label: string; value: string; visible: boolean }) => {
                        let select = state.components.select;

                        // delete any existing items with the same group or where group is empty
                        for(let i = 0; i < select.length; i++) {
                                if(select[i].group === payload.group) {
                                        delete select[i];
                                        select = select.filter((a) => { return typeof a !== 'undefined' });
                                } else if(select[i].group === '') {
                                        delete select[i]; select = select.filter((a) => { return typeof a !== 'undefined' });
                                }
                        }

                        select.push(payload);
                        state.components.select = select;
                },
                deployAssociation: (state, payload: string) => {
                        const assoc = state.advancedOptions.associations[payload as keyof {}] as { components: string[]; visible: boolean[] }
                        if(assoc) {
                                const associations = assoc.components;
                                const visible = state.advancedOptions.associations[payload as keyof {}]['visible'] as boolean[];
                                const components = state.components;
                                // if preset associations for payload group are found
                                if(associations) {
                                        // loop through all associations
                                        for(let i = 0; i < associations.length; i++){
                                                // loop through all components
                                                for(const [, componentValue] of Object.entries(components)) {
                                                        // loop through each item in each component
                                                        for(let j = 0; j < componentValue.length; j++) {
                                                                // if group matches association then set visible to true
                                                                if('visible' in componentValue[j]) {
                                                                        if(componentValue[j].group == associations[i]) {
                                                                                componentValue[j].visible = visible[i];
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
                addFiles({ commit, state }, payload: { type: string; selection: FileList }) {
                        if (payload.type === 'input') {
                                const promise = new Promise(() => { commit('setInputFiles', payload.selection) });
                                
                                promise.then(
                                        function(result) { console.log('addFiles', result, ' :: files:', state.inputFiles) }, 
                                        function(err) { console.log('addFiles', err, ' :: files: ', state.inputFiles) }
                                );
                        } else if (payload.type === 'output') {
                                const promise = new Promise(() => { commit('setOutputFiles', payload.selection) });
                                
                                promise.then(
                                        function(result) { console.log('addFiles', result, ' :: files:', state.outputFiles) }, 
                                        function(err) { console.log('addFiles', err, ' :: files: ', state.outputFiles) }
                                );
                        }
                },

                setAppIsDragEnter({ getters, commit }, payload: boolean) {
                        const promise = new Promise((resolve, reject) => {
                                commit('setAppIsDragEnter', payload);
                                if(getters.appIsDragEnter === payload) resolve(true)
                                else resolve(reject);
                        });
                        if(promise) console.log('Store :: setAppIsDragEnter: ', payload);
                },

                setAdvancedOptionsIsShown({ getters, commit }, payload: boolean) {
                        const promise = new Promise((resolve, reject) => {
                                commit('setAdvancedOptionsIsShown', payload);
                                if(getters.advancedOptionsIsShown === payload) resolve(true);
                                else resolve(reject);
                        });

                        if(promise) console.log('Store :: setAdvancedOptionsIsShown: ', payload);
                },

                syncModeWithRadioGroup({ getters, commit }, payload: {group: string; id: string}) {
                        const radios = getters.componentsRadio;
                        for(let i = 0; i < radios.length; i++) {
                                if(radios[i].group === payload.group) {
                                        for(let j = 0; j < radios[i].id.length; j++) {
                                                if(radios[i].id[j] === payload.id && radios[i].checked[j] === true) {
                                                        commit('setAdvancedOptionsMode', payload.id);
                                                }
                                        }
                                }
                        }
                },
        }
});

export default store;
