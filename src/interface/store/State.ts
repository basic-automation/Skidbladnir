import { AppComponent } from './AppComponent';
import { ComponentGroup } from './ComponentGroup';
import { AdvancedOptions } from './AdvancedOptions';

export interface State {
        inputFiles: {};
        outputFiles: {};
        app: AppComponent;
        components: ComponentGroup;
        advancedOptions: AdvancedOptions;
}