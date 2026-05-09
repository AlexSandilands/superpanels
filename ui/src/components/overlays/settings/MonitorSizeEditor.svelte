<script lang="ts">
  import { api, errorMessage, type Monitor } from '$lib/api';
  import { monitorStore } from '$lib/stores/monitors.svelte';
  import { profileStore } from '$lib/stores/profile.svelte';
  import { toast } from '$lib/stores/toast.svelte';
  import StepperInput from '../../chrome/StepperInput.svelte';

  type Props = {
    monitor: Monitor;
    onClose: () => void;
  };

  let { monitor, onClose }: Props = $props();

  const aspect = $derived(monitor.resolution[0] / monitor.resolution[1]);

  function diagonalFromMm(w: number, h: number): number {
    return Math.sqrt(w * w + h * h) / 25.4;
  }

  const round1dp = (v: number) => Math.round(v * 10) / 10;

  function mmFromDiagonal(diagIn: number, asp: number): [number, number] {
    const diagMm = diagIn * 25.4;
    const h = diagMm / Math.sqrt(1 + asp * asp);
    const w = asp * h;
    return [round1dp(w), round1dp(h)];
  }

  // svelte-ignore state_referenced_locally
  const initialMm: [number, number] = monitor.physical_size_mm ?? mmFromDiagonal(27, aspect);
  let widthMm = $state<number>(initialMm[0]);
  let heightMm = $state<number>(initialMm[1]);
  let diagonalIn = $state<number>(diagonalFromMm(initialMm[0], initialMm[1]));
  let saving = $state(false);

  function setDiagonal(v: number) {
    diagonalIn = v;
    const [w, h] = mmFromDiagonal(v, aspect);
    widthMm = w;
    heightMm = h;
  }

  function setWidth(v: number) {
    widthMm = round1dp(v);
    diagonalIn = round1dp(diagonalFromMm(widthMm, heightMm));
  }

  function setHeight(v: number) {
    heightMm = round1dp(v);
    diagonalIn = round1dp(diagonalFromMm(widthMm, heightMm));
  }

  async function save() {
    if (widthMm <= 0 || heightMm <= 0) {
      toast.error('Invalid size', 'width and height must be > 0');
      return;
    }
    saving = true;
    try {
      await api.setMonitorPhysicalSize({ stableId: monitor.stable_id, name: monitor.name }, [
        widthMm,
        heightMm,
      ]);
      await monitorStore.refresh();
      const detail = `${widthMm.toFixed(1)}×${heightMm.toFixed(1)} mm`;
      const active = profileStore.activeName;
      if (active) {
        toast.success('Physical size saved', {
          detail,
          action: {
            label: 'Re-apply now',
            onClick: () => {
              void api.applyProfile(active).catch((err: unknown) => {
                toast.error('Re-apply failed', errorMessage(err));
              });
            },
          },
        });
      } else {
        toast.success('Physical size saved', detail);
      }
      onClose();
    } catch (err) {
      toast.error('Could not save size', errorMessage(err));
    } finally {
      saving = false;
    }
  }
</script>

<div class="editor">
  <div class="row">
    <StepperInput
      label="diag"
      unit="in"
      value={diagonalIn}
      onChange={setDiagonal}
      step={0.5}
      bigStep={1}
      min={1}
      max={120}
      decimals={1}
      width={48}
    />
    <span class="sep">→</span>
    <StepperInput
      label="W"
      unit="mm"
      value={widthMm}
      onChange={setWidth}
      step={0.5}
      bigStep={10}
      min={1}
      max={5000}
      decimals={1}
      width={56}
    />
    <StepperInput
      label="H"
      unit="mm"
      value={heightMm}
      onChange={setHeight}
      step={0.5}
      bigStep={10}
      min={1}
      max={5000}
      decimals={1}
      width={56}
    />
  </div>
  <div class="actions">
    <span class="hint mono">aspect from {monitor.resolution[0]}×{monitor.resolution[1]}</span>
    <div style:flex="1"></div>
    <button class="btn ghost sm" onclick={onClose} disabled={saving}>Cancel</button>
    <button class="btn sm" onclick={save} disabled={saving}>
      {saving ? 'Saving…' : 'Save'}
    </button>
  </div>
</div>

<style>
  .editor {
    margin-top: 8px;
    padding: 12px 14px;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .sep {
    color: var(--text-3);
    font-size: 11px;
    padding: 0 2px;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .hint {
    font-size: 10px;
    color: var(--text-3);
  }
</style>
