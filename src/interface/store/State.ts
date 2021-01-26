import { AppComponent } from './AppComponent';
import { ComponentGroup } from './ComponentGroup';
import { AdvancedOptions } from './AdvancedOptions';
import { InputFiles } from './InputFiles';
import { OutputFiles } from './OutputFiles';

export interface State {
        inputFiles: InputFiles;
        outputFiles: OutputFiles;
        app: AppComponent;
        components: ComponentGroup;
        advancedOptions: AdvancedOptions;
}