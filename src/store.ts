import { createStore } from 'vuex';

export const store = createStore({
        state: {
                inputFiles: {},
                outputFiles: {},
                app: {
                        isDragEnter: false,
                        canDragEnter: true,
                },
                advancedOptions: {
                        isShown: false,
                },
        },

        getters: {
                inputFiles: state => { return state.inputFiles },
                outputFiles: state => { return state.outputFiles },
                appIsDragEnter: state => { return state.app.isDragEnter },
                appCanDragEnter: state => { return state.app.canDragEnter },
                advancedOptionsIsShown: state => { return state.advancedOptions.isShown },
        },

        mutations: {
                setInputFiles: (state, payload: File) => state.inputFiles = payload,
                setOutputFiles: (state, payload: File) => state.outputFiles = payload,
                setAppIsDragEnter: (state, payload: boolean) => state.app.isDragEnter = payload,
                setAppCanDragEnter: (state, payload: boolean) => state.app.canDragEnter = payload,
                setAdvancedOptionsIsShown: (state, payload: boolean) => state.advancedOptions.isShown = payload,
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
