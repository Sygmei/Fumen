import type {
    LoginLinkResponse,
    UserResponse,
} from "$backend/models";
import AccountModal from "./AccountModal.svelte";
import AnnotationModal from "./AnnotationModal.svelte";
import AppConfigModal from "./AppConfigModal.svelte";
import ConfirmModal from "./ConfirmModal.svelte";
import CredentialModal from "./CredentialModal.svelte";
import ScannerModal from "./ScannerModal.svelte";
import { setActiveModal, type ModalSettings } from "./modalState";

export interface ConfirmOptions {
    title?: string;
    message?: string;
    confirmText?: string;
    cancelText?: string;
    variant?: "default" | "danger" | "warning" | "success";
    onConfirm: () => void | Promise<void>;
    onCancel?: () => void | Promise<void>;
}

export interface CredentialModalOptions {
    title: string;
    loadLink: () => Promise<LoginLinkResponse>;
    eyebrow?: string;
    linkLabel?: string;
}

export interface ScannerModalOptions {
    onConnectToken: (token: string) => Promise<void>;
}

export interface AccountModalOptions {
    currentUser: UserResponse;
    onSaved: (user: UserResponse) => void;
}

export interface AppConfigModalOptions {
    enableCountIn: boolean;
    countInMeasures: number;
    onToggleCountIn: (value: boolean) => void;
    onChangeCountInMeasures: (value: number) => void;
}

export interface AnnotationModalOptions {
    positionLabel: string;
    initialComment?: string;
    onSave: (comment: string) => void | Promise<void>;
}

export function showModal(
    modal: ModalSettings["modal"],
    props: ModalSettings["props"] = {},
) {
    setActiveModal({
        modal,
        props,
    });
}

export function showConfirmModal(options: ConfirmOptions) {
    showModal(ConfirmModal, {
        title: options.title ?? "Confirm Action",
        message: options.message ?? "",
        confirmText: options.confirmText ?? "Confirm",
        cancelText: options.cancelText ?? "Cancel",
        variant: options.variant ?? "default",
        onConfirm: options.onConfirm,
        onCancel: options.onCancel ?? (() => {}),
    });
}

export function showDeleteConfirmModal(
    itemName: string,
    onConfirm: () => void | Promise<void>,
    onCancel?: () => void | Promise<void>,
) {
    showConfirmModal({
        title: `Delete ${itemName}`,
        message: `This action cannot be undone. Are you sure you want to delete ${itemName}?`,
        confirmText: "Delete",
        variant: "danger",
        onConfirm,
        onCancel,
    });
}

export function showCredentialModal(options: CredentialModalOptions) {
    showModal(CredentialModal, {
        title: options.title,
        loadLink: options.loadLink,
        eyebrow: options.eyebrow ?? "Temporary access",
        linkLabel: options.linkLabel ?? "Connection link",
    });
}

export function showScannerModal(options: ScannerModalOptions) {
    showModal(ScannerModal, {
        onConnectToken: options.onConnectToken,
    });
}

export function showAccountModal(options: AccountModalOptions) {
    showModal(AccountModal, {
        currentUser: options.currentUser,
        onSaved: options.onSaved,
    });
}

export function showAppConfigModal(options: AppConfigModalOptions) {
    showModal(AppConfigModal, {
        enableCountIn: options.enableCountIn,
        countInMeasures: options.countInMeasures,
        onToggleCountIn: options.onToggleCountIn,
        onChangeCountInMeasures: options.onChangeCountInMeasures,
    });
}

export function showAnnotationModal(options: AnnotationModalOptions) {
    showModal(AnnotationModal, {
        positionLabel: options.positionLabel,
        initialComment: options.initialComment ?? "",
        onSave: options.onSave,
    });
}
