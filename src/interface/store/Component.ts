export interface Component {
        category: string;
        group: string;
        id?: string[];

        association?: string[];
        dependency?: string;

        label?: string;
        value?: string | number | boolean[];
        valueMin?: number;
        valueMax?: number;

        isOpen?: boolean;
        visible: boolean;
}