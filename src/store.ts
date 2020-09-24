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
                                jpegLike: false,
                                lossy: false,
                                lossless: false,
                                nearLossless: false,
                                presets: false,
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
                        if(payload === 'jpegLike') state.advancedOptions.mode = { jpegLike: true, lossy: false, lossless: false, nearLossless: false, presets: false }
                        else if(payload === 'lossy') state.advancedOptions.mode = { jpegLike: false, lossy: true, lossless: false, nearLossless: false, presets: false }
                        else if(payload === 'lossless') state.advancedOptions.mode = { jpegLike: false, lossy: false, lossless: true, nearLossless: false, presets: false }
                        else if(payload === 'nearLossless') state.advancedOptions.mode = { jpegLike: false, lossy: false, lossless: false, nearLossless: true, presets: false }
                        else if(payload === 'presets') state.advancedOptions.mode = { jpegLike: false, lossy: false, lossless: false, nearLossless: false, presets: true }
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
        }
});

export default store;
