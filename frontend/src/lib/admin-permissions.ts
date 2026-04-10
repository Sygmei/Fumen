import type { AdminMusic, AppUser, Ensemble, EnsembleRole, GlobalRole } from "./api";

export function canAccessAdmin(user: AppUser | null): boolean {
    return !!user && user.role !== "user";
}

export function isSuperadmin(user: AppUser | null): boolean {
    return user?.role === "superadmin";
}

export function hasGlobalPower(user: AppUser | null): boolean {
    return user?.role === "superadmin" || user?.role === "admin";
}

export function canManageUsers(user: AppUser | null): boolean {
    return (
        user?.role === "superadmin" ||
        user?.role === "admin" ||
        user?.role === "manager"
    );
}

export function canCreateEnsembles(user: AppUser | null): boolean {
    return canManageUsers(user);
}

export function canUseUsersSection(user: AppUser | null): boolean {
    return canManageUsers(user);
}

export function canManageEnsembleMembers(ensemble: Ensemble, user: AppUser | null): boolean {
    return (
        !!user &&
        (hasGlobalPower(user) || user.managed_ensemble_ids.includes(ensemble.id))
    );
}

export function canDeleteUserAccount(user: AppUser, actor: AppUser | null): boolean {
    if (!actor || user.id === actor.id) return false;
    if (actor.role === "superadmin") return user.role !== "superadmin";
    if (actor.role === "admin") return user.role !== "admin" && user.role !== "superadmin";
    if (actor.role === "manager") return user.role === "user" && user.created_by_user_id === actor.id;
    return false;
}

export function canDeleteEnsembleRecord(ensemble: Ensemble, actor: AppUser | null): boolean {
    if (!actor) return false;
    return (
        hasGlobalPower(actor) ||
        ensemble.created_by_user_id === actor.id ||
        actor.managed_ensemble_ids.includes(ensemble.id)
    );
}

export function canDeleteScore(music: AdminMusic, actor: AppUser | null): boolean {
    if (!actor) return false;
    return hasGlobalPower(actor) || music.owner_user_id === actor.id;
}

export function canEditOwnedScore(music: AdminMusic, actor: AppUser | null): boolean {
    return canDeleteScore(music, actor);
}

export function canManageScoreEnsembles(music: AdminMusic, actor: AppUser | null): boolean {
    if (!actor) return false;
    return (
        hasGlobalPower(actor) ||
        actor.role === "manager" ||
        music.owner_user_id === actor.id
    );
}

export function canManageEnsembleScores(ensemble: Ensemble, actor: AppUser | null): boolean {
    if (!actor) return false;
    return (
        hasGlobalPower(actor) ||
        actor.role === "manager" ||
        actor.managed_ensemble_ids.includes(ensemble.id) ||
        actor.editable_ensemble_ids.includes(ensemble.id)
    );
}

export function allowedCreateRoles(actor: AppUser | null): Array<Exclude<GlobalRole, "superadmin">> {
    if (!actor) return ["user"];
    if (actor.role === "superadmin" || actor.role === "admin") {
        return ["admin", "manager", "editor", "user"];
    }
    if (actor.role === "manager") return ["user"];
    return ["user"];
}

export function defaultCreateRole(actor: AppUser | null): Exclude<GlobalRole, "superadmin"> {
    const roles = allowedCreateRoles(actor);
    return roles.includes("user") ? "user" : (roles[0] ?? "user");
}

export function allowedEnsembleRolesForUser(user: AppUser, actor: AppUser | null): EnsembleRole[] {
    if (user.role === "superadmin" || user.role === "admin") return [];
    if (user.role === "manager") {
        return actor?.role === "manager" ? [] : ["manager", "editor", "user"];
    }
    if (user.role === "editor") return ["editor", "user"];
    return ["user"];
}
