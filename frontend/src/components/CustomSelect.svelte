<script lang="ts">
    import { tick } from "svelte";
    import type { Component } from "svelte";
    import { portal } from "../lib/portal";
    import { ChevronDown, Check } from "@lucide/svelte";

    type SelectOption = {
        value: string;
        label: string;
        description?: string;
        icon?: string;
        iconComponent?: Component<{ size?: number; strokeWidth?: number }>;
        tone?: string;
    };

    let {
        value = $bindable(),
        options,
        label,
        placeholder = "Select an option",
        disabled = false,
        compact = false,
        showDescriptionInTrigger = false,
        onValueChange,
    }: {
        value: string;
        options: SelectOption[];
        label?: string;
        placeholder?: string;
        disabled?: boolean;
        compact?: boolean;
        showDescriptionInTrigger?: boolean;
        onValueChange?: (value: string) => void;
    } = $props();

    let open = $state(false);
    let highlightIndex = $state(-1);
    let rootElement = $state<HTMLElement | null>(null);
    let menuElement = $state<HTMLElement | null>(null);
    let openUpward = $state(false);
    let menuMaxHeight = $state(320);
    let menuLeft = $state(0);
    let menuWidth = $state(0);
    let menuTop = $state<number | null>(null);
    let menuBottom = $state<number | null>(null);
    let isMobileSheet = $state(false);

    const listboxId = `custom-select-${Math.random().toString(36).slice(2, 10)}`;

    let selectedOption = $derived.by(
        () => options.find((option) => option.value === value) ?? null,
    );

    function scrollHighlightedOptionIntoView() {
        if (!menuElement || highlightIndex < 0) {
            return;
        }

        const highlighted = menuElement.querySelector<HTMLElement>(
            `[data-option-index="${highlightIndex}"]`,
        );
        highlighted?.scrollIntoView({ block: "nearest" });
    }

    function openMenu() {
        if (disabled || options.length === 0) {
            return;
        }

        open = true;
        const selectedIndex = options.findIndex(
            (option) => option.value === value,
        );
        highlightIndex = selectedIndex >= 0 ? selectedIndex : 0;
        void tick().then(scrollHighlightedOptionIntoView);
    }

    function closeMenu() {
        open = false;
        highlightIndex = -1;
        openUpward = false;
        isMobileSheet = false;
    }

    function toggleMenu() {
        if (open) {
            closeMenu();
        } else {
            openMenu();
        }
    }

    function selectOption(option: SelectOption) {
        value = option.value;
        onValueChange?.(option.value);
        closeMenu();
    }

    function moveHighlight(direction: 1 | -1) {
        if (!open) {
            openMenu();
            return;
        }

        if (options.length === 0) {
            return;
        }

        const baseIndex = highlightIndex >= 0 ? highlightIndex : 0;
        highlightIndex =
            (baseIndex + direction + options.length) % options.length;
        void tick().then(scrollHighlightedOptionIntoView);
    }

    function handleTriggerKeydown(event: KeyboardEvent) {
        if (disabled) {
            return;
        }

        if (event.key === "ArrowDown") {
            event.preventDefault();
            moveHighlight(1);
            return;
        }

        if (event.key === "ArrowUp") {
            event.preventDefault();
            moveHighlight(-1);
            return;
        }

        if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            if (open && highlightIndex >= 0) {
                selectOption(options[highlightIndex]);
            } else {
                openMenu();
            }
            return;
        }

        if (event.key === "Escape" && open) {
            event.preventDefault();
            closeMenu();
        }
    }

    function handleOptionMouseEnter(index: number) {
        highlightIndex = index;
    }

    function handleWindowPointerDown(event: PointerEvent) {
        if (!open || !rootElement) {
            return;
        }

        const target = event.target;
        if (
            target instanceof Node &&
            !rootElement.contains(target) &&
            !menuElement?.contains(target)
        ) {
            closeMenu();
        }
    }

    function toneClass(option: SelectOption | null) {
        return option?.tone ? `tone-${option.tone}` : "";
    }

    async function updateMenuPlacement() {
        if (!open || !rootElement) {
            return;
        }

        await tick();

        const rect = rootElement.getBoundingClientRect();
        const viewport = window.visualViewport;
        const viewportHeight = viewport?.height ?? window.innerHeight;
        const viewportWidth = viewport?.width ?? window.innerWidth;
        const margin = viewportWidth <= 640 ? 12 : 16;
        const gap = viewportWidth <= 640 ? 8 : 8;

        isMobileSheet = viewportWidth <= 640;

        const availableBelow = Math.max(
            140,
            viewportHeight - rect.bottom - margin - gap,
        );
        const availableAbove = Math.max(
            140,
            rect.top - margin - gap,
        );
        const preferredHeight = Math.min(menuElement?.scrollHeight ?? 320, 360);
        const nextMenuWidth = Math.min(
            Math.max(rect.width, compact ? 224 : rect.width),
            viewportWidth - margin * 2,
        );

        openUpward =
            availableBelow < preferredHeight && availableAbove > availableBelow;
        menuMaxHeight = Math.max(
            140,
            Math.min(openUpward ? availableAbove : availableBelow, 360),
        );
        menuWidth = nextMenuWidth;
        menuLeft = Math.min(
            Math.max(margin, rect.left),
            Math.max(
                margin,
                viewportWidth - nextMenuWidth - margin,
            ),
        );
        menuTop = openUpward ? null : rect.bottom + gap;
        menuBottom = openUpward ? viewportHeight - rect.top + gap : null;
    }

    function handleViewportChange() {
        if (open) {
            void updateMenuPlacement();
        }
    }

    $effect(() => {
        if (open) {
            void updateMenuPlacement();
        }
    });

    $effect(() => {
        const viewport = window.visualViewport;
        if (!viewport) {
            return;
        }

        viewport.addEventListener("resize", handleViewportChange);
        viewport.addEventListener("scroll", handleViewportChange);

        return () => {
            viewport.removeEventListener("resize", handleViewportChange);
            viewport.removeEventListener("scroll", handleViewportChange);
        };
    });
</script>

<svelte:window
    onpointerdown={handleWindowPointerDown}
    onresize={handleViewportChange}
    onscroll={handleViewportChange}
/>

<div class="custom-select" bind:this={rootElement}>
    {#if label}
        <span class="custom-select-label">{label}</span>
    {/if}

    <button
        type="button"
        class="custom-select-trigger"
        class:is-open={open}
        class:is-compact={compact}
        {disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listboxId}
        onclick={toggleMenu}
        onkeydown={handleTriggerKeydown}
    >
        <span
            class={`custom-select-trigger-icon ${toneClass(selectedOption)}`}
            aria-hidden="true"
        >
            {#if selectedOption?.iconComponent}
                <selectedOption.iconComponent size={18} strokeWidth={2} />
            {:else}
                {selectedOption?.icon ?? ""}
            {/if}
        </span>
        <span class="custom-select-trigger-copy">
            <strong>{selectedOption?.label ?? placeholder}</strong>
            {#if showDescriptionInTrigger && selectedOption?.description}
                <small>{selectedOption.description}</small>
            {/if}
        </span>
        <span class="custom-select-trigger-chevron" aria-hidden="true">
            <ChevronDown size={14} strokeWidth={2.2} />
        </span>
    </button>

    {#if open}
        <div
            class="custom-select-menu-wrap"
            class:opens-upward={openUpward}
            class:is-mobile-sheet={isMobileSheet}
            use:portal
            style={`left: ${menuLeft}px; width: ${menuWidth}px; ${
                menuTop === null ? "" : `top: ${menuTop}px;`
            } ${menuBottom === null ? "" : `bottom: ${menuBottom}px;`}`}
        >
            <div
                class="custom-select-menu"
                class:is-mobile-sheet={isMobileSheet}
                bind:this={menuElement}
                role="listbox"
                id={listboxId}
                tabindex="-1"
                style={`max-height: ${menuMaxHeight}px;`}
            >
                {#each options as option, index}
                    <button
                        type="button"
                        class="custom-select-option"
                        class:is-selected={option.value === value}
                        class:is-highlighted={index === highlightIndex}
                        class:is-compact={compact}
                        role="option"
                        aria-selected={option.value === value}
                        data-option-index={index}
                        onclick={() => selectOption(option)}
                        onmouseenter={() => handleOptionMouseEnter(index)}
                    >
                        <span
                            class={`custom-select-option-icon ${toneClass(option)}`}
                            aria-hidden="true"
                        >
                            {#if option.iconComponent}
                                <option.iconComponent size={18} strokeWidth={2} />
                            {:else}
                                {option.icon ?? ""}
                            {/if}
                        </span>
                        <span class="custom-select-option-copy">
                            <strong>{option.label}</strong>
                        </span>
                        <span
                            class="custom-select-option-check"
                            aria-hidden="true"
                        >
                            {#if option.value === value}
                                <Check size={16} strokeWidth={2.4} />
                            {/if}
                        </span>
                    </button>
                {/each}
            </div>
        </div>
    {/if}
</div>

<style>
    .custom-select {
        position: relative;
        display: grid;
        gap: 10px;
        --custom-select-trigger-bg: var(--surface);
        --custom-select-panel-bg: var(--surface);
        --custom-select-hover-bg: var(--accent-faint);
        --custom-select-selected-bg: var(--accent-dim);
        --custom-select-border: color-mix(
            in srgb,
            var(--border-strong) 88%,
            white 12%
        );
        --custom-select-border-strong: color-mix(
            in srgb,
            var(--accent) 22%,
            var(--border-strong) 78%
        );
        --custom-select-panel-border: color-mix(
            in srgb,
            var(--border-strong) 92%,
            white 8%
        );
        --custom-select-icon-bg: color-mix(
            in srgb,
            var(--surface-alt) 74%,
            white 26%
        );
        --custom-select-icon-border: color-mix(
            in srgb,
            var(--border-strong) 68%,
            white 32%
        );
    }

    @media (prefers-color-scheme: dark) {
        .custom-select {
            --custom-select-trigger-bg: var(--surface);
            --custom-select-panel-bg: var(--surface);
            --custom-select-hover-bg: var(--accent-faint);
            --custom-select-selected-bg: var(--accent-dim);
            --custom-select-border: color-mix(
                in srgb,
                var(--border-strong) 86%,
                white 14%
            );
            --custom-select-border-strong: color-mix(
                in srgb,
                var(--accent) 30%,
                var(--border-dark-strong) 70%
            );
            --custom-select-panel-border: color-mix(
                in srgb,
                var(--border-dark-strong) 88%,
                white 12%
            );
            --custom-select-icon-bg: color-mix(
                in srgb,
                var(--surface-alt) 78%,
                black 22%
            );
            --custom-select-icon-border: color-mix(
                in srgb,
                var(--border-dark-strong) 82%,
                white 18%
            );
        }
    }

    .custom-select-label {
        font-family: "Fira Code", "Cascadia Code", monospace;
        font-size: 0.68rem;
        font-weight: 500;
        letter-spacing: 0.14em;
        text-transform: uppercase;
        color: var(--accent);
    }

    .custom-select-trigger {
        position: relative;
        display: grid;
        grid-template-columns: auto minmax(0, 1fr) auto;
        align-items: center;
        gap: 12px;
        width: 100%;
        height: var(--control-height, 48px);
        min-height: var(--control-height, 48px);
        padding: 10px 12px 10px 10px;
        border: 1px solid var(--custom-select-border);
        background: var(--custom-select-trigger-bg);
        box-shadow: none;
        appearance: none;
        color: var(--text);
        text-align: left;
        cursor: pointer;
        overflow: hidden;
        transition:
            border-color 160ms ease,
            transform 160ms ease;
    }

    .custom-select-trigger:hover,
    .custom-select-trigger:focus-visible,
    .custom-select-trigger.is-open {
        border-color: var(--custom-select-border-strong);
        box-shadow: none;
        transform: none;
    }

    .custom-select-trigger:focus-visible {
        outline: 3px solid var(--accent-dim);
        outline-offset: -1px;
    }

    .custom-select-trigger:disabled {
        cursor: not-allowed;
        opacity: 0.6;
        transform: none;
        box-shadow: none;
    }

    .custom-select-trigger.is-compact {
        min-height: 36px;
        padding: 4px 8px 4px 6px;
        gap: 8px;
    }

    .custom-select-trigger-icon,
    .custom-select-option-icon {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 28px;
        height: 28px;
        background: var(--surface-alt);
        color: var(--accent);
        border: 1px solid var(--border-strong);
        box-shadow: none;
        font-size: 0.82rem;
        line-height: 1;
        flex-shrink: 0;
    }

    .custom-select-trigger.is-compact .custom-select-trigger-icon,
    .custom-select-option.is-compact .custom-select-option-icon {
        width: 22px;
        height: 22px;
        font-size: 0.72rem;
    }

    .custom-select-trigger-icon.tone-admin,
    .custom-select-option-icon.tone-admin {
        color: var(--accent);
        border-color: var(--accent);
        background: var(--surface-alt);
    }

    .custom-select-trigger-icon.tone-manager,
    .custom-select-option-icon.tone-manager {
        color: var(--accent);
        border-color: var(--border-strong);
        background: var(--surface-alt);
    }

    .custom-select-trigger-icon.tone-editor,
    .custom-select-option-icon.tone-editor {
        color: var(--accent);
        border-color: var(--border-strong);
        background: var(--surface-alt);
    }

    .custom-select-trigger-copy,
    .custom-select-option-copy {
        display: grid;
        gap: 1px;
        min-width: 0;
    }

    .custom-select-trigger-copy strong,
    .custom-select-option-copy strong {
        font-size: 0.88rem;
        font-weight: 700;
        letter-spacing: 0.01em;
        line-height: 1.15;
        overflow-wrap: anywhere;
    }

    .custom-select-trigger.is-compact .custom-select-trigger-copy strong,
    .custom-select-option.is-compact .custom-select-option-copy strong {
        font-size: 0.78rem;
    }

    .custom-select-trigger-copy small {
        color: var(--text-dim);
        font-size: 0.68rem;
        line-height: 1.2;
        overflow-wrap: anywhere;
    }

    .custom-select-trigger.is-compact .custom-select-trigger-copy small {
        font-size: 0.64rem;
    }

    .custom-select-trigger-chevron {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 28px;
        height: 28px;
        border: 1px solid transparent;
        color: var(--text-dim);
        transition:
            transform 160ms ease,
            color 160ms ease,
            border-color 160ms ease,
            background 160ms ease;
    }

    .custom-select-trigger.is-open .custom-select-trigger-chevron {
        transform: rotate(180deg);
    }

    .custom-select-trigger:hover .custom-select-trigger-chevron,
    .custom-select-trigger:focus-visible .custom-select-trigger-chevron,
    .custom-select-trigger.is-open .custom-select-trigger-chevron {
        color: var(--accent);
        border-color: color-mix(in srgb, var(--accent) 18%, transparent);
        background: var(--accent-faint);
    }

    .custom-select-menu-wrap {
        position: fixed;
        z-index: 1200;
    }

    .custom-select-menu-wrap.opens-upward {
        top: auto;
    }

    .custom-select-menu-wrap.is-mobile-sheet {
        z-index: 1300;
    }

    .custom-select-menu {
        display: grid;
        gap: 0;
        padding: 0;
        overflow-y: auto;
        border: 1px solid var(--border-strong);
        background-color: var(--surface);
        scrollbar-color: var(--border-strong) transparent;
        animation: custom-select-panel-in 150ms ease;
    }

    .custom-select-menu.is-mobile-sheet {
        padding: 0;
        border-color: color-mix(in srgb, var(--accent) 24%, var(--border-strong));
    }

    .custom-select-option {
        position: relative;
        display: grid;
        grid-template-columns: auto minmax(0, 1fr) auto;
        align-items: center;
        gap: 8px;
        width: 100%;
        height: var(--control-height, 48px);
        min-height: var(--control-height, 48px);
        padding: 10px 14px;
        border: 1px solid transparent;
        background: transparent;
        color: var(--text);
        text-align: left;
        cursor: pointer;
        overflow: hidden;
        transition:
            border-color 140ms ease,
            background 140ms ease,
            transform 140ms ease;
    }

    .custom-select-option::before {
        content: "";
        position: absolute;
        inset: 0 auto 0 0;
        width: 3px;
        background: var(--accent);
        opacity: 0;
        transition: opacity 140ms ease;
    }

    .custom-select-option.is-compact {
        gap: 6px;
        min-height: 36px;
        padding: 5px 12px;
    }

    .custom-select-option + .custom-select-option {
        border-top-color: color-mix(in srgb, var(--border-strong) 72%, transparent);
    }

    .custom-select-option.is-highlighted,
    .custom-select-option:hover,
    .custom-select-option:focus-visible {
        border-color: color-mix(in srgb, var(--accent) 16%, var(--border));
        background: var(--accent-faint);
        outline: none;
    }

    .custom-select-option.is-selected {
        border-color: color-mix(in srgb, var(--accent) 26%, var(--border));
        background: var(--accent-dim);
    }

    .custom-select-option.is-highlighted::before,
    .custom-select-option:hover::before,
    .custom-select-option:focus-visible::before,
    .custom-select-option.is-selected::before {
        opacity: 1;
    }

    .custom-select-option-check {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 16px;
        height: 16px;
        color: var(--accent);
        flex-shrink: 0;
    }

    @keyframes custom-select-panel-in {
        from {
            opacity: 0;
            transform: translateY(6px);
        }

        to {
            opacity: 1;
            transform: translateY(0);
        }
    }

    @media (max-width: 640px) {
        .custom-select {
            gap: 8px;
        }

        .custom-select-label {
            font-size: 0.64rem;
        }

        .custom-select-trigger {
            height: var(--control-height, 48px);
            min-height: var(--control-height, 48px);
            gap: 10px;
            padding: 10px 10px 10px 8px;
        }

        .custom-select-trigger-copy strong,
        .custom-select-option-copy strong {
            font-size: 0.82rem;
        }

        .custom-select-trigger-copy small {
            font-size: 0.64rem;
        }

        .custom-select-menu.is-mobile-sheet {
            padding: 0;
        }

        .custom-select-option {
            height: var(--control-height, 48px);
            min-height: var(--control-height, 48px);
            gap: 7px;
            padding: 10px 12px;
        }
    }
</style>
