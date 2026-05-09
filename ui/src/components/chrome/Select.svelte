<script lang="ts">
  type Option = { value: string; label: string };
  type Props = {
    value: string;
    options: ReadonlyArray<Option>;
    onChange: (v: string) => void;
    minWidth?: number;
  };
  let { value, options, onChange, minWidth = 0 }: Props = $props();

  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return {
      destroy() {
        node.remove();
      },
    };
  }

  let buttonEl: HTMLButtonElement | undefined = $state();
  let menuEl: HTMLDivElement | undefined = $state();
  let open = $state(false);
  let pos = $state({ left: 0, top: 0, width: 0 });
  let activeIdx = $state(0);

  const currentLabel = $derived(options.find((o) => o.value === value)?.label ?? '');

  function openMenu() {
    if (!buttonEl) return;
    const rect = buttonEl.getBoundingClientRect();
    const idx = options.findIndex((o) => o.value === value);
    activeIdx = idx >= 0 ? idx : 0;
    pos = { left: rect.left, top: rect.bottom + 4, width: rect.width };
    open = true;
  }
  function closeMenu() {
    open = false;
    buttonEl?.focus();
  }
  function pick(v: string) {
    onChange(v);
    closeMenu();
  }

  function onKey(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      closeMenu();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      activeIdx = (activeIdx + 1) % options.length;
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      activeIdx = (activeIdx - 1 + options.length) % options.length;
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const opt = options[activeIdx];
      if (opt) pick(opt.value);
    } else if (e.key === 'Home') {
      e.preventDefault();
      activeIdx = 0;
    } else if (e.key === 'End') {
      e.preventDefault();
      activeIdx = options.length - 1;
    }
  }

  function onButtonKey(e: KeyboardEvent) {
    if (e.key === 'ArrowDown' || e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      openMenu();
    }
  }

  $effect(() => {
    if (!open || !menuEl || !buttonEl) return;
    const rect = menuEl.getBoundingClientRect();
    const btnRect = buttonEl.getBoundingClientRect();
    const margin = 6;
    let top = btnRect.bottom + 4;
    if (top + rect.height + margin > window.innerHeight) {
      top = btnRect.top - rect.height - 4;
    }
    const left = Math.min(btnRect.left, window.innerWidth - rect.width - margin);
    pos = { left: Math.max(margin, left), top: Math.max(margin, top), width: btnRect.width };
  });
</script>

<svelte:window onkeydown={onKey} />

<button
  bind:this={buttonEl}
  type="button"
  class="field ui trigger"
  style:min-width="{minWidth}px"
  aria-haspopup="listbox"
  aria-expanded={open}
  onclick={() => (open ? closeMenu() : openMenu())}
  onkeydown={onButtonKey}
>
  <span class="lbl">{currentLabel}</span>
  <span class="caret" aria-hidden="true"></span>
</button>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div use:portal class="fixed inset-0" style:z-index="60" onclick={closeMenu}></div>
  <div
    use:portal
    bind:this={menuEl}
    class="panel menu"
    role="listbox"
    style:left="{pos.left}px"
    style:top="{pos.top}px"
    style:min-width="{pos.width}px"
    style:z-index="61"
  >
    {#each options as opt, i (opt.value)}
      <button
        type="button"
        class="row"
        class:active={i === activeIdx}
        role="option"
        aria-selected={opt.value === value}
        onmouseenter={() => (activeIdx = i)}
        onclick={() => pick(opt.value)}
      >
        <span class="check">{opt.value === value ? '✓' : ''}</span>
        <span class="lbl">{opt.label}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .trigger {
    display: inline-flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    cursor: default;
    text-align: left;
    transition:
      background-color 80ms ease,
      border-color 80ms ease;
  }
  .trigger:hover {
    border-color: var(--line-2);
    background: var(--panel-2);
  }
  .caret {
    display: inline-block;
    width: 10px;
    height: 6px;
    flex-shrink: 0;
    background-color: var(--text-2);
    -webkit-mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='6' viewBox='0 0 10 6'%3E%3Cpath fill='none' stroke='black' stroke-width='1.5' stroke-linecap='round' stroke-linejoin='round' d='M1 1l4 4 4-4'/%3E%3C/svg%3E")
      no-repeat center / contain;
    mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='6' viewBox='0 0 10 6'%3E%3Cpath fill='none' stroke='black' stroke-width='1.5' stroke-linecap='round' stroke-linejoin='round' d='M1 1l4 4 4-4'/%3E%3C/svg%3E")
      no-repeat center / contain;
  }
  .menu {
    position: fixed;
    padding: 4px;
    max-height: 280px;
    overflow-y: auto;
  }
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px 6px 6px;
    border-radius: 5px;
    border: none;
    background: transparent;
    color: inherit;
    text-align: left;
    font-size: 12px;
    font-family: inherit;
    cursor: default;
  }
  .row.active {
    background: var(--panel-2);
  }
  .check {
    width: 14px;
    text-align: center;
    color: var(--accent);
    font-size: 11px;
    flex-shrink: 0;
  }
  .lbl {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
