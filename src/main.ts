import { createApp } from 'vue';
import App from './App.vue';
import './main.css';
import { store } from './store';
import FileUpload from './components/FileUpload.vue';
import AdvancedOptionsToggle from './components/AdvancedOptionsToggle.vue'

const app = createApp(App);
app.component('FileUpload', FileUpload);
app.component('AdvancedOptionsToggle', AdvancedOptionsToggle);
app.use(store);
app.mount('#app');
