<!-- eslint-disable max-len -->
<template>
	<div :class="[{ 'bg-gray-100 pointer-events-none': isDropZone }, 'flex flex-1 h-full min-h-20 md:min-h-10 md:h-10 flex-col md:flex-row justify-center items-center transition-all ease-in-out duration-500']" @dragenter.prevent="fileUploadDragEnter()">
		<div :class="{ 'hidden pointer-events-none': isDropZone, 'flex w-full flex-row justify-center items-center h-10 rounded-full bg-gray-400 transition-all ease-in-out duration-500 flex-shrink': !isDropZone, 'ba-error': isError }" @dragover.prevent @dragleave.prevent>
			<label :class="{ 'hidden pointer-events-none': isDropZone, 'ml-6 uppercase text-xs my-auto align-middle antialiased pointer-events-none': !isDropZone }" @dragover.prevent @dragleave.prevent>{{ label }}</label>
			<input v-model="input" disabled type="text" :class="{ 'hidden pointer-events-none': isDropZone, 'bg-gray-400 flex-1 h-10 ml-4 rounded-full rounded-l-none mr-2 focus:outline-none bg-transparent text-gray-800 text-xs transition-all ease-in-out duration-500 flex-shrink w-full': !isDropZone }" @dragover.prevent @dragleave.prevent @dragenter.prevent />
		</div>
		<button :class="{ 'hidden pointer-events-none': isDropZone, 'flex md:ml-4 mt-4 md:mt-0 h-10 uppercase rounded-full glow-xl w-full md:w-32 md:min-w-32 text-gray-400 text-xs justify-center items-center focus:outline-none focus:ba-inner-shaddow-sm hover:glow-sm transition-all ease-in-out duration-1000 antialiased flex-shrink-0': !isDropZone }" @click="browseClick()" @dragover.prevent @dragleave.prevent>Browse</button>
		<div :class="{ 'hidden': !isDropZone, 'flex md:h-full min-h-10 justify-center items-center flex-1 text-gray-800 uppercase font-bold text-xs pointer-events-auto z-50': isDropZone }" @dragover.prevent @dragleave="dropZoneDragLeave()" @drop.prevent="fileChangeHandler(true, $event), fileUploadDropFile()">
			<p :class="{ 'pointer-events-none': isDropZone }">{{ dropZoneLabel }}</p>
		</div>
	</div>
	<input type="file" ref="file" class="hidden pointer-events-none" @change="fileChangeHandler(false, $event)" :multiple="isInput()" :webkitdirectory="isOutput()" />
</template>

<script lang="ts">
/* eslint-disable max-len */
import { defineComponent } from "vue";
import store from "../store";
import { State } from '../interface/store/State'; 
import { AppComponent } from '../interface/store/AppComponent';
const { dialog } = require('electron').remote;

export default defineComponent({
        name: "FileUpload",

        props: {
                label: String,
                dropZoneLabel: String,
                type: {
                        type: String,
                        required: true,
                },
        },

        data() {
                return {
                        isInput: (): boolean => { return this.type === "input" ? true : false; },
                        isOutput: (): boolean => { return this.type === "output" ? true : false; },
                        input: "",
                        isDropZone: false,
                        isFileUploadDragEnter: false,
                        isFileUploadDropFile: false,
                        isDropZoneDragEnter: false,
                        isDropZoneDragLeave: false,
                        isError: false,
                };
        },

        computed: {
                appIsDragEnter: () => store.getters.appIsDragEnter as AppComponent['isDragEnter'],
                appCanDragEnter: () => store.getters.appCanDragEnter as AppComponent['canDragEnter'],
                inputFiles: () => store.getters.inputFiles as State['inputFiles'],
                outputFiles: () => store.getters.outputFiles as State['outputFiles'],
        },

        setup() { return { store }; },

        methods: {
                browseClick() {
                        if(this.type === 'input') {
                                const fileInput = this.$refs.file as HTMLInputElement;
                                fileInput.click();
                        } else {
                                dialog.showOpenDialog({
                                        properties: ['openDirectory', 'multiSelections']
                                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                                }).then((result: any) => {
                                        this.fileChangeHandler(false, undefined, result.filePaths);
                                });
                        }
                },

                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                fileChangeHandler(fromDropZone: boolean, event?: any, path?: string[]) {
                        // if neither path nor event are passed return false
                        if(typeof path !== 'undefined' && typeof event !== 'undefined') return false;

                        //define vars
                        let selection: string[] = [];

                        //if this is an input selector
                        if(this.isInput()) {
                                //if the file was selected via drap and drop use datatransfer api else use files api
                                if(fromDropZone) {
                                        for(const file of event.dataTransfer.files) selection.push(file.path);
                                }
                                else {
                                        for(const file of event.target.files) selection.push(file.path);
                                }
                                
                        } else if(typeof path !== 'undefined') selection = path;

                        //construct payload
                        const payload = { type: this.type, paths: selection };

                        //update the store
                        this.store.dispatch("addFiles",payload);
                },

                fileUploadDragEnter() { if(!this.isFileUploadDragEnter) this.isFileUploadDragEnter = true; },

                fileUploadDropFile() { if (!this.isFileUploadDropFile) this.isFileUploadDropFile = true; },

                dropZoneDragEnter() { if (!this.isDropZoneDragEnter) this.isDropZoneDragEnter = true; },
                
                dropZoneDragLeave() { if (!this.isDropZoneDragEnter) this.isDropZoneDragLeave = true; },
        },

        watch: {
                isFileUploadDragEnter: function () {
                        if (this.isFileUploadDragEnter) {
                                store.dispatch("setAppIsDragEnter", false);
                                store.commit("setAppCanDragEnter", false);
                                this.isDropZone = true;
                        }
                },

                isDropZoneDragLeave: function () {
                        if (this.isDropZoneDragLeave) store.commit("setAppCanDragEnter", true);

                        if (this.isDropZone && this.appCanDragEnter) {
                                this.isDropZoneDragEnter = false;
                                this.isDropZoneDragLeave = false;
                                this.isFileUploadDragEnter = false;
                                this.isDropZone = false;
                        }
                },

                isFileUploadDropFile: function () {
                        this.isDropZoneDragEnter = false;
                        this.isDropZoneDragLeave = false;
                        this.isFileUploadDragEnter = false;
                        this.isFileUploadDropFile = false;
                        this.isDropZone = false;
                },

                inputFiles: {
                        deep: true,
                        handler() {
                                if(this.type === 'input') {
                                        this.input = '';
                                        const newInput = [];
                                        for(const [, fileValue] of Object.entries(this.inputFiles)) newInput.push(fileValue);
                                        this.input = newInput.join(', ');

                                        //old
                                        /* for(let i = 0; i < this.inputFiles.length; i++) {
                                                if(i === this.inputFiles.length - 1) this.input += this.inputFiles[i];
                                                else this.input += this.inputFiles[i] + ', ';
                                        } */
                                        //old
                                }
                                
                        }
                },

                outputFiles: {
                        deep: true,
                        handler() {
                                if(this.type === 'output') {
                                        this.input = '';
                                        const newInput = [];
                                        for(const [, fileValue] of Object.entries(this.outputFiles)) newInput.push(fileValue);
                                        this.input = newInput.join(', ');

                                        //old
                                        /* for(let i = 0; i < this.outputFiles.length; i++) {
                                                if(i === this.outputFiles.length - 1) this.input += this.outputFiles[i];
                                                else this.input += this.outputFiles[i] + ', ';
                                        } */
                                        //old
                                }
                        }
                },
        },
});
</script>

<!-- eslint-disable max-len -->
<style scoped>
	@media (min-width: 640px) {
		.sm\:min-w-32 {
			min-width: 8rem;
		}
	}

	.focus\:glow-none:focus {
		box-shadow: 0 0px 5px rgba(148, 163, 184, 0), 0 0px 10px -5px  rgba(148, 163, 184, 0);
        }
        
	.focus\:ba-inner-shaddow-sm:focus {
		box-shadow: inset 0 0px 5px rgba(0, 0, 0, 0.35), 0 0px 10px -5px rgba(0, 0, 0, 0.5), 0 0px 5px rgba(160, 174, 192, 0.35), 0 00px 10px -5px rgba(160, 174, 192, 0.5);
        }
        
	.hover\:glow-sm:hover {
		box-shadow: 0 0px 5px  rgba(148, 163, 184, .35), 0 0px 10px -5px rgba(148, 163, 184, .5);
        }
        
	.glow-xl {
		box-shadow: 0 0px 5px rgba(148, 163, 184, .7), 0 0px 10px -5px rgba(148, 163, 184, 1);
        }
        
	.ba-error {
		box-shadow: 0 0px 10px rgba(245, 101, 101, 0.7), 0 0px 20px -5px rgba(245, 101, 101, 1);
	}
</style>