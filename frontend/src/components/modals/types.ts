import type { Component } from "svelte";
import type {
    AdminMusicResponse as AdminMusic,
    UserResponse as AppUser,
} from "../../adapters/fumen-backend/src/models";
import type { EnsembleRole, GlobalRole } from "../../lib/roles";
import type { StemQualityProfile } from "../../lib/stem-quality";

export interface SelectOption {
    value: string;
    label: string;
    description?: string;
    icon?: string;
    iconComponent?: Component<{ size?: number; strokeWidth?: number }>;
    tone?: string;
}

export interface GlobalRoleOption extends SelectOption {
    value: Exclude<GlobalRole, "superadmin">;
}

export interface EnsembleRoleOption extends SelectOption {
    value: EnsembleRole;
}

export interface UserEditDraft {
    displayName: string;
    role: Exclude<GlobalRole, "superadmin">;
    avatarFile: File | null;
    avatarPreview: string | null;
    clearAvatar: boolean;
}

export interface EnsembleMemberAssignment {
    userId: string;
    role: EnsembleRole;
}

export interface UploadScoreDraft {
    title: string;
    publicId: string;
    qualityProfile: StemQualityProfile;
    file: File | null;
    iconFile: File | null;
    ensembleIds: string[];
}

export interface EditScoreDraft {
    title: string;
    publicId: string;
    icon: string;
    iconFile: File | null;
}

export interface ScoreEnsemblePickerSelection {
    mode: "upload" | "score";
    music?: AdminMusic;
    initialSelectedEnsembleIds: string[];
}

export interface RolePickerTarget {
    kind: "member" | "invite";
    user: AppUser;
    value: EnsembleRole;
}
