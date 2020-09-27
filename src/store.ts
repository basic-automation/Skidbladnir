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
                        radio: [
                                {
                                        group: '',
                                        id: [''],
                                        checked: [false],
                                }
                        ],
                },
                advancedOptions: {
                        isShown: false,
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
                                presets: {
                                        checked: false,
                                        label: 'Specify a set of pre-defined parameters to suit a particular type of source material. Possible values are: default, photo, picture, drawing, icon, text. Since preset overwrites the other parameters values (except the quality one), this option should preferably appear first in the order of the arguments.',
                                },
                        }
                },
        },

        getters: {
                inputFiles: state => { return state.inputFiles },
                outputFiles: state => { return state.outputFiles },
                appIsDragEnter: state => { return state.app.isDragEnter },
                appCanDragEnter: state => { return state.app.canDragEnter },
                componentsRadio: state => { return state.components.radio },
                advancedOptions: state => { return state.advancedOptions },
                advancedOptionsIsShown: state => { return state.advancedOptions.isShown },
                advancedOptionsMode: state => { return state.advancedOptions.mode },
                advancedOptionsModeJPEGLike: state => { return state.advancedOptions.mode.jpegLike },
                advancedOptionsModeLossy: state => { return state.advancedOptions.mode.lossy },
                advancedOptionsModeLossless: state => { return state.advancedOptions.mode.lossless },
                advancedOptionsModeNearLossless: state => { return state.advancedOptions.mode.nearLossless },
                advancedOptionsModeNearPresets: state => { return state.advancedOptions.mode.presets },
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
                registerComponentRadioGroup: (state, payload: { group: string; id: string[]; checked: boolean[] }) => {
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
                                        console.log('Register Component Radio Group: splice(' + i +', 1)');
                                }
                                i++;
                        });

                        radio.push(mygroup);
                        state.components.radio = radio;
                },
        },

        actions: {
                addFiles({ commit, state }, payload: { type: string; selection: FileList }) {
                        console.log("Store: addfiles: payload", payload)
                        console.log("Store: addfiles: payload: type", payload.type);
                        console.log("Store: addfiles: payload: files", payload.selection);
                        if (payload.type === 'input') {
                                const promise = new Promise(() => { commit('setInputFiles', payload.selection) });
                                
                                promise.then(
                                        function(result) {
                                                console.log('addFiles', result, ' :: files:', state.inputFiles); // "Stuff worked!"
                                        }, 
                                        function(err) {
                                                console.log('addFiles', err, ' :: files: ', state.inputFiles); // Error: "It broke"
                                        }
                                );
                        } else if (payload.type === 'output') {
                                const promise = new Promise(() => { commit('setOutputFiles', payload.selection) });
                                
                                promise.then(
                                        function(result) {
                                                console.log('addFiles', result, ' :: files:', state.outputFiles); // "Stuff worked!"
                                        }, 
                                        function(err) {
                                                console.log('addFiles', err, ' :: files: ', state.outputFiles); // Error: "It broke"
                                        }
                                );
                        }
                },

                setAppIsDragEnter({ getters, commit }, payload: boolean) {
                        const promise = new Promise((resolve, reject) => {
                                commit('setAppIsDragEnter', payload);
                                if(getters.appIsDragOver === payload) resolve(true)
                                else resolve(reject);
                        });

                        if(promise){
                                console.log('Store :: setAppIsDragEnter: ', payload);
                        }
                },

                setAdvancedOptionsIsShown({ getters, commit }, payload: boolean) {
                        const promise = new Promise((resolve, reject) => {
                                commit('setAdvancedOptionsIsShown', payload);
                                if(getters.advancedOptionsIsShown === payload) resolve(true)
                                else resolve(reject);
                        });

                        if(promise){
                                console.log('Store :: setAdvancedOptionsIsShown: ', payload);
                        }
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
