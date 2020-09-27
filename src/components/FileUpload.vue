<!-- eslint-disable max-len -->
<template>
	<div class="flex flex-1 h-full min-h-20 md:min-h-full md:h-10 flex-col md:flex-row justify-center items-center transition-all ease-in-out duration-500" @dragenter.prevent="fileUploadDragEnter()" :class="{ 'bg-gray-500 pointer-events-none': isDropZone }">
		<div :class="{ 'hidden pointer-events-none': isDropZone, 'flex w-full flex-row justify-center items-center h-10 rounded-full bg-gray-500 transition-all ease-in-out duration-500': !isDropZone, 'ba-error': isError }" @dragover.prevent @dragleave.prevent>
			<div :class="{ 'hidden pointer-events-none': isDropZone, 'ml-6 uppercase text-xs my-auto align-middle antialiased': !isDropZone }" @dragover.prevent @dragleave.prevent>{{ label }}</div>
			<input v-model="input" disabled type="text" :class="{ 'hidden pointer-events-none': isDropZone, 'flex-1 h-10 ml-4 rounded-full rounded-l-none mr-2 focus:outline-none bg-transparent text-gray-900 text-xs transition-all ease-in-out duration-500': !isDropZone }" @dragover.prevent @dragleave.prevent @dragenter.prevent />
		</div>
		<button :class="{ 'hidden pointer-events-none': isDropZone, 'flex md:ml-4 mt-4 md:mt-0 h-10 uppercase rounded-full glow-xl w-full md:w-32 md:min-w-32 text-gray-500 text-xs justify-center items-center focus:outline-none focus:ba-inner-shaddow-sm hover:glow-sm transition-all ease-in-out duration-1000 antialiased': !isDropZone }" @click="$refs.file.click()" @dragover.prevent @dragleave.prevent>Browse</button>
		<div :class="{ 'hidden': !isDropZone, 'flex md:h-full min-h-10 justify-center items-center flex-1 text-gray-900 uppercase font-bold text-xs pointer-events-auto z-50': isDropZone }" @dragover.prevent @dragleave="dropZoneDragLeave()" @drop.prevent="fileChangeHandler($event, type, true), fileUploadDropFile()">
			<p :class="{ 'pointer-events-none': isDropZone }">{{ dropZoneLabel }}</p>
		</div>
	</div>
	<input type="file" ref="file" class="hidden pointer-events-none" @change="fileChangeHandler($event, type, false)" :multiple="isInput" :directory="isOutput" />
</template>

<script lang="ts">
/* eslint-disable max-len */
import { defineComponent } from "vue";
import store from "../store";
import fs from "fs";

export default defineComponent({
        name: "FileUpload",

        props: {
                label: String,
                dropZoneLabel: String,
                type: String,
        },

        data() {
                return {
                        isInput: () => { if(this.type === "input") return true; else return false; },
                        isOutput: () => { if(this.type === "output") return true; else return false; },
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
                appIsDragEnter: () => { return store.getters.appIsDragEnter; },
                appCanDragEnter: () => { return store.getters.appCanDragEnter; },
        },

        setup() { return { store }; },

        methods: {
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                fileChangeHandler(event: any, type: string, fromDropZone: boolean) {
                        let selection: FileList;
                        if(!fromDropZone) selection = event.target.files; else selection = event.dataTransfer.files;
                        if(selection.length > 0) {
                                this.input = "";
                                for (let i = 0; i <= selection.length - 1; i++) {
                                        if (selection[i].path) this.input += selection[i].path; else this.input += selection[i].name;
                                        if (i != selection.length - 1) this.input += ", ";
                                }
                                const payload = { type, selection };
                                if(this.type === "input") {
                                        try {
                                                this.store.dispatch("addFiles", payload);
                                                this.isError = false;
                                        } catch (error) {
                                                this.input = error;
                                                this.isError = true;
                                        }
                                } else if (this.type === "output") {
                                        try {
                                                let checksOut = false;
                                                for (let i = 0; i <= selection.length - 1; i++) {
                                                        console.log(fs.existsSync(selection[i].path));
                                                        console.log(fs.lstatSync(selection[i].path).isDirectory());
                                                        if(fs.existsSync(selection[i].path) && fs.lstatSync(selection[i].path).isDirectory()) checksOut = true; 
                                                }
                                                if(checksOut){
                                                        this.store.dispatch("addFiles",payload);
                                                        this.isError = false;
                                                } else {
                                                        throw "Must select one or more folders."
                                                }
                                        } catch (error) {
                                                this.input = error;
                                                this.isError = true;
                                        }
                                }
                        }
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
                        if (this.isDropZoneDragLeave) {
                                store.commit("setAppCanDragEnter", true);
                        }
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

	@media (min-width: 768px) {
		.md\:min-h-20 {
			min-height: 5rem;
		}
	}

	.min-h-20 {
		min-height: 5rem;
	}

	.min-h-10 {
		min-height: 2.5rem;
	}

	.focus\:glow-none:focus {
		box-shadow: 0 0px 5px rgba(160, 174, 192, 0), 0 0px 10px -5px rgba(160, 174, 192, 0);
	}
	.focus\:ba-inner-shaddow-sm:focus {
		box-shadow: inset 0 0px 5px rgba(0, 0, 0, 0.35), 0 0px 10px -5px rgba(0, 0, 0, 0.5), 0 0px 5px rgba(160, 174, 192, 0.35), 0 00px 10px -5px rgba(160, 174, 192, 0.5);
	}
	.hover\:glow-sm:hover {
		box-shadow: 0 0px 5px rgba(160, 174, 192, 0.35), 0 0px 10px -5px rgba(160, 174, 192, 0.5);
	}
	.glow-xl {
		box-shadow: 0 0px 5px rgba(160, 174, 192, 0.7), 0 0px 10px -5px rgba(160, 174, 192, 1);
	}
	.ba-error {
		box-shadow: 0 0px 10px rgba(245, 101, 101, 0.7), 0 0px 20px -5px rgba(245, 101, 101, 1);
	}
</style>
