import { derived, writable } from "svelte/store";

export interface ModalSettings {
    id?: string;
    modal: any;
    props: Record<string, unknown>;
}

export const modalStack = writable<ModalSettings[]>([]);

function createModalId() {
    if ("randomUUID" in crypto) {
        return crypto.randomUUID();
    }

    return `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

function withId(settings: ModalSettings): ModalSettings {
    return {
        ...settings,
        id: settings.id ?? createModalId(),
    };
}

export function setActiveModal(settings: ModalSettings) {
    modalStack.set([withId(settings)]);
}

export function pushModal(settings: ModalSettings) {
    modalStack.update((stack) => [...stack, withId(settings)]);
}

export function closeModal() {
    modalStack.update((stack) => stack.slice(0, -1));
}

export function clearModals() {
    modalStack.set([]);
}

export const activeModal = derived(
    modalStack,
    (stack) => stack[stack.length - 1] ?? null,
);
