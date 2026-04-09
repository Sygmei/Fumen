<script lang="ts">
    import { tick } from "svelte";

    type SelectOption = {
        value: string;
        label: string;
        description?: string;
        icon?: string;
        tone?: string;
    };

    let {
        value = $bindable(),
        options,
        label,
        placeholder = "Select an option",
        disabled = false,
        compact = false,
        showDescriptionInTrigger = true,
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

    const listboxId = `custom-select-${Math.random().toString(36).slice(2, 10)}`;

    let selectedOption = $derived.by(
        () => options.find((option) => option.value === value) ?? null,
    );

    function openMenu() {
        if (disabled || options.length === 0) {
            return;
        }

        open = true;
        const selectedIndex = options.findIndex((option) => option.value === value);
        highlightIndex = selectedIndex >= 0 ? selectedIndex : 0;
    }

    function closeMenu() {
        open = false;
        highlightIndex = -1;
        openUpward = false;
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
        highlightIndex = (baseIndex + direction + options.length) % options.length;
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

        if (event.key === "Escape") {
            if (open) {
                event.preventDefault();
                closeMenu();
            }
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
        if (target instanceof Node && !rootElement.contains(target)) {
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
        const viewportHeight = window.visualViewport?.height ?? window.innerHeight;
        const margin = 16;
        const gap = 8;
        const availableBelow = Math.max(140, viewportHeight - rect.bottom - margin - gap);
        const availableAbove = Math.max(140, rect.top - margin - gap);
        const preferredHeight = Math.min(menuElement?.scrollHeight ?? 320, 360);

        openUpward = availableBelow < preferredHeight && availableAbove > availableBelow;
        menuMaxHeight = Math.max(
            140,
            Math.min(openUpward ? availableAbove : availableBelow, 360),
        );
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
        disabled={disabled}
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
            {selectedOption?.icon ?? "•"}
        </span>
        <span class="custom-select-trigger-copy">
            <strong>{selectedOption?.label ?? placeholder}</strong>
            {#if showDescriptionInTrigger && selectedOption?.description}
                <small>{selectedOption.description}</small>
            {/if}
        </span>
        <span class="custom-select-trigger-chevron" aria-hidden="true">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                <path d="m6 9 6 6 6-6" />
            </svg>
        </span>
    </button>

    {#if open}
        <div class="custom-select-menu-wrap" class:opens-upward={openUpward}>
            <div
                class="custom-select-menu"
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
                        onclick={() => selectOption(option)}
                        onmouseenter={() => handleOptionMouseEnter(index)}
                    >
                        <span
                            class={`custom-select-option-icon ${toneClass(option)}`}
                            aria-hidden="true"
                        >
                            {option.icon ?? "•"}
                        </span>
                        <span class="custom-select-option-copy">
                            <strong>{option.label}</strong>
                            {#if option.description}
                                <small>{option.description}</small>
                            {/if}
                        </span>
                        <span class="custom-select-option-check" aria-hidden="true">
                            {#if option.value === value}
                                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round">
                                    <path d="M20 6 9 17l-5-5" />
                                </svg>
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
        gap: 8px;
        --custom-select-trigger-bg:
            linear-gradient(135deg, rgba(255, 255, 255, 0.97), rgba(244, 247, 252, 0.94)),
            var(--surface-alt);
        --custom-select-panel-bg: rgba(255, 252, 250, 0.97);
        --custom-select-hover-bg: rgba(255, 255, 255, 0.82);
        --custom-select-selected-bg:
            linear-gradient(135deg, rgba(255, 255, 255, 0.96), color-mix(in srgb, var(--accent) 8%, white 92%));
        --custom-select-border: color-mix(in srgb, var(--border) 84%, white 16%);
        --custom-select-panel-border: rgba(18, 32, 56, 0.1);
        --custom-select-shadow: 0 14px 32px rgba(18, 32, 56, 0.08);
        --custom-select-shadow-strong: 0 26px 60px rgba(18, 32, 56, 0.18);
    }

    @media (prefers-color-scheme: dark) {
        .custom-select {
            --custom-select-trigger-bg:
                linear-gradient(
                    135deg,
                    color-mix(in srgb, var(--surface) 88%, black 12%),
                    color-mix(in srgb, var(--surface-alt) 92%, black 8%)
                );
            --custom-select-panel-bg: color-mix(in srgb, var(--surface-dark-2) 88%, black 12%);
            --custom-select-hover-bg: color-mix(in srgb, var(--surface-dark-3) 88%, white 12%);
            --custom-select-selected-bg:
                linear-gradient(
                    135deg,
                    color-mix(in srgb, var(--surface-dark-2) 80%, var(--accent) 20%),
                    color-mix(in srgb, var(--surface-dark-3) 90%, black 10%)
                );
            --custom-select-border: color-mix(in srgb, var(--border-strong) 78%, white 22%);
            --custom-select-panel-border: var(--border-dark-strong);
            --custom-select-shadow: 0 16px 36px rgba(0, 0, 0, 0.34);
            --custom-select-shadow-strong: 0 28px 68px rgba(0, 0, 0, 0.46);
        }
    }

    .custom-select-label {
        font-size: 0.8rem;
        font-weight: 700;
        letter-spacing: 0.02em;
    }

    .custom-select-trigger {
        display: grid;
        grid-template-columns: auto minmax(0, 1fr) auto;
        align-items: center;
        gap: 10px;
        width: 100%;
        padding: 8px 10px 8px 8px;
        border: 1px solid var(--custom-select-border);
        border-radius: 16px;
        background: var(--custom-select-trigger-bg);
        box-shadow: var(--custom-select-shadow);
        color: var(--text);
        text-align: left;
        transition:
            border-color 160ms ease,
            box-shadow 160ms ease,
            transform 160ms ease;
    }

    .custom-select-trigger:hover,
    .custom-select-trigger:focus-visible,
    .custom-select-trigger.is-open {
        border-color: color-mix(in srgb, var(--accent) 42%, white 58%);
        box-shadow:
            var(--custom-select-shadow),
            0 0 0 4px rgba(196, 43, 13, 0.08);
    }

    .custom-select-trigger:focus-visible {
        outline: none;
    }

    .custom-select-trigger.is-compact {
        padding: 6px 8px 6px 6px;
        gap: 8px;
        border-radius: 14px;
    }

    .custom-select-trigger-icon,
    .custom-select-option-icon {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 42px;
        height: 42px;
        border-radius: 14px;
        background: color-mix(in srgb, var(--surface) 78%, var(--surface-alt) 22%);
        color: color-mix(in srgb, var(--text-dim) 78%, var(--text) 22%);
        border: 1px solid color-mix(in srgb, var(--border-strong) 72%, transparent 28%);
        box-shadow: inset 0 1px 0 color-mix(in srgb, white 28%, transparent 72%);
        font-size: 1.05rem;
        line-height: 1;
    }

    .custom-select-trigger.is-compact .custom-select-trigger-icon,
    .custom-select-option.is-compact .custom-select-option-icon {
        width: 34px;
        height: 34px;
        border-radius: 11px;
        font-size: 0.92rem;
    }

    .custom-select-trigger-icon.tone-admin,
    .custom-select-option-icon.tone-admin {
        color: #184fae;
        background: linear-gradient(135deg, rgba(210, 229, 255, 0.98), rgba(238, 245, 255, 0.9));
    }

    .custom-select-trigger-icon.tone-manager,
    .custom-select-option-icon.tone-manager {
        color: #0e7b68;
        background: linear-gradient(135deg, rgba(201, 247, 234, 0.98), rgba(238, 255, 248, 0.92));
    }

    .custom-select-trigger-icon.tone-editor,
    .custom-select-option-icon.tone-editor {
        color: #9a4a07;
        background: linear-gradient(135deg, rgba(255, 227, 198, 0.98), rgba(255, 244, 231, 0.92));
    }

    .custom-select-trigger-copy,
    .custom-select-option-copy {
        display: grid;
        gap: 3px;
        min-width: 0;
    }

    .custom-select-trigger-copy strong,
    .custom-select-option-copy strong {
        font-size: 0.94rem;
        letter-spacing: 0.01em;
    }

    .custom-select-trigger.is-compact .custom-select-trigger-copy strong,
    .custom-select-option.is-compact .custom-select-option-copy strong {
        font-size: 0.84rem;
    }

    .custom-select-trigger-copy small,
    .custom-select-option-copy small {
        color: var(--text-dim);
        font-size: 0.77rem;
        line-height: 1.4;
    }

    .custom-select-trigger.is-compact .custom-select-trigger-copy small,
    .custom-select-option.is-compact .custom-select-option-copy small {
        font-size: 0.72rem;
    }

    .custom-select-trigger-chevron {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        color: var(--text-dim);
        transition: transform 160ms ease;
    }

    .custom-select-trigger.is-open .custom-select-trigger-chevron {
        transform: rotate(180deg);
    }

    .custom-select-menu-wrap {
        position: absolute;
        top: calc(100% + 8px);
        left: 0;
        right: 0;
        z-index: 25;
    }

    .custom-select-menu-wrap.opens-upward {
        top: auto;
        bottom: calc(100% + 8px);
    }

    .custom-select-menu {
        display: grid;
        gap: 8px;
        padding: 10px;
        overflow-y: auto;
        border-radius: 20px;
        border: 1px solid var(--custom-select-panel-border);
        background: var(--custom-select-panel-bg);
        backdrop-filter: blur(20px);
        box-shadow: var(--custom-select-shadow-strong);
        scrollbar-color: var(--border-strong) transparent;
    }

    .custom-select-option {
        display: grid;
        grid-template-columns: auto minmax(0, 1fr) auto;
        align-items: center;
        gap: 12px;
        width: 100%;
        padding: 10px;
        border: 1px solid transparent;
        border-radius: 16px;
        background: transparent;
        color: var(--text);
        text-align: left;
        transition:
            border-color 140ms ease,
            background 140ms ease,
            transform 140ms ease;
    }

    .custom-select-option.is-compact {
        gap: 10px;
        padding: 8px;
        border-radius: 14px;
    }

    .custom-select-option.is-highlighted,
    .custom-select-option:hover,
    .custom-select-option:focus-visible {
        border-color: rgba(196, 43, 13, 0.14);
        background: var(--custom-select-hover-bg);
        transform: translateY(-1px);
        outline: none;
    }

    .custom-select-option.is-selected {
        border-color: color-mix(in srgb, var(--accent) 34%, white 66%);
        background: var(--custom-select-selected-bg);
    }

    .custom-select-option-check {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 18px;
        height: 18px;
        color: var(--accent);
    }
</style>