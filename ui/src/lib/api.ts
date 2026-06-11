// Thin wrappers around `invoke` so the UI talks to typed commands and never
// reaches into Tauri directly. Every Tauri command exposed by the backend
// has a wrapper here.

import { invoke } from '@tauri-apps/api/core';
import type { IpcError } from './types/IpcError';
import type { LibraryFilter } from './types/LibraryFilter';
import type { PreviewArgs } from './types/PreviewArgs';
import type { Profile } from './types/profile-helpers';
import type { MonitorPlacement } from './types/MonitorPlacement';
import type { ProfileValidity } from './types/ProfileValidity';
import type { Schedule } from './types/Schedule';
import type { SpanSource } from './types/SpanSource';

export type { Profile };

// `Monitor`, `Profile`, `Config`, `CropSpec`, `LibraryEntry`, `RuntimeState`
// have richer Rust shapes than we need to mirror here. Until ts-rs covers
// them, keep them as `unknown`-ish records and narrow at the call site.
export type Monitor = {
  id: number;
  name: string;
  stable_id: string | null;
  position: [number, number];
  resolution: [number, number];
  physical_size_mm: [number, number] | null;
  scale: number;
  rotation: 'none' | 'right' | 'inverted' | 'left';
  refresh_hz: number | null;
  ppi: number | null;
};

export type LibraryEntry = {
  path: string;
  resolution: [number, number];
  aspect_ratio: number;
  file_size: number;
  modified: { secs_since_epoch: number; nanos_since_epoch: number } | string | number;
  tags: string[];
  favourite: boolean;
  last_shown: { secs_since_epoch: number; nanos_since_epoch: number } | string | number | null;
  show_count: number;
};

export type RuntimeState = {
  version: number;
  active_profile: string | null;
  slideshow: {
    current_index: number | null;
    history_len: number;
    paused: boolean;
    current_path: string | null;
    remaining_secs: number | null;
    pool_len: number | null;
  } | null;
  last_apply_unix_secs: number | null;
  last_apply_backend?: string | null;
};

export type TrayIconStyle = 'white' | 'blue';

export type AppliedReport = {
  monitors_set?: number;
  backend?: string;
  elapsed_ms?: number;
};

type IpcErrorHook = (err: unknown) => void;
let ipcErrorHook: IpcErrorHook | null = null;

/** Register a handler invoked on every IPC rejection. Used by the
 *  daemon-status store so the banner can react to `DaemonUnreachable`
 *  errors without `api.ts` needing to import the store (which would
 *  introduce a circular dep). One handler at a time — last writer wins. */
export function setIpcErrorHook(handler: IpcErrorHook | null): void {
  ipcErrorHook = handler;
}

async function call<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  try {
    return await invoke<T>(name, args);
  } catch (err) {
    ipcErrorHook?.(err);
    throw err;
  }
}

export const api = {
  detectMonitors: () => call<Monitor[]>('detect_monitors'),
  redetect: () => call<{ monitors: number }>('redetect'),
  setMonitorPhysicalSize: (
    identifier: { stableId?: string | null; name?: string | null },
    physicalMm: [number, number],
  ) =>
    call<void>('set_monitor_physical_size', {
      stableId: identifier.stableId ?? null,
      name: identifier.name ?? null,
      physicalMm,
    }),
  listProfiles: () =>
    call<{
      profiles: Profile[];
      validity: { profile: string; validity: ProfileValidity }[];
    }>('list_profiles'),
  applyProfile: (name: string) => call<AppliedReport>('apply_profile', { name }),
  applyCanvas: (profile: Profile, activeName: string | null) =>
    call<AppliedReport>('apply_canvas', { profile, activeName }),
  saveProfile: (profile: Profile, opts?: { recomputeTopology?: boolean }) =>
    call<void>('save_profile', {
      profile,
      recomputeTopology: opts?.recomputeTopology ?? false,
    }),
  deleteProfile: (name: string) => call<void>('delete_profile', { name }),
  duplicateProfile: (name: string, newName: string) =>
    call<void>('duplicate_profile', { name, newName }),
  renameProfile: (name: string, newName: string) => call<void>('rename_profile', { name, newName }),
  updateProfileMonitorState: (profile: string, stableId: string, placement: MonitorPlacement) =>
    call<void>('update_profile_monitor_state', { profile, stableId, placement }),
  updateProfileImageTransform: (
    profile: string,
    imageRectMm: { x_mm: number; y_mm: number; w_mm: number; h_mm: number } | null,
  ) => call<void>('update_profile_image_transform', { profile, image_rect_mm: imageRectMm }),
  updateProfileSource: (profile: string, source: SpanSource) =>
    call<void>('update_profile_source', { profile, source }),
  listSchedules: () =>
    call<{
      schedules: Schedule[];
      paused: boolean;
    }>('list_schedules'),
  saveSchedules: (schedules: Schedule[]) => call<void>('save_schedules', { schedules }),
  setSchedulesPaused: (paused: boolean) =>
    call<{ paused: boolean }>('set_schedules_paused', { paused }),
  previewCrop: (args: PreviewArgs) => call<unknown>('preview_crop', { args }),
  libraryList: (filter: LibraryFilter) => call<LibraryEntry[]>('library_list', { filter }),
  libraryThumbnail: (path: string) =>
    call<{ data: string; mime: string }>('library_thumbnail', { path }),
  // Local-only render path used by the canvas preview for any selected /
  // dropped source file — bypasses library-roots gating.
  sourceThumbnail: (path: string) =>
    call<{ data: string; mime: string }>('source_thumbnail', { path }),
  libraryTag: (path: string, tag: string, on: boolean) =>
    call<void>('library_tag', { path, tag, on }),
  libraryDelete: (path: string) => call<void>('library_delete', { path }),
  libraryRescan: () => call<{ count: number }>('library_rescan'),
  slideshowNext: () => call<AppliedReport>('slideshow_next'),
  slideshowPrev: () => call<AppliedReport>('slideshow_prev'),
  slideshowGoto: (path: string) => call<AppliedReport>('slideshow_goto', { path }),
  slideshowPause: (paused?: boolean) =>
    call<{ paused: boolean }>('slideshow_pause', paused === undefined ? {} : { paused }),
  getConfig: () => call<unknown>('get_config'),
  saveConfig: (config: unknown) => call<void>('save_config', { config }),
  currentState: () => call<RuntimeState>('current_state'),
  setAutostart: (enabled: boolean) => call<{ enabled: boolean }>('set_autostart', { enabled }),
  getAutostart: () => call<{ enabled: boolean }>('get_autostart'),
  setTrayIconStyle: (style: TrayIconStyle) =>
    call<{ style: TrayIconStyle }>('set_tray_icon_style', { style }),
  getTrayIconStyle: () => call<{ style: TrayIconStyle }>('get_tray_icon_style'),
  daemonStatus: () => call<{ connected: boolean }>('daemon_status'),
  startDaemon: () => call<{ exe: string }>('start_daemon'),
};

export type ApiError = IpcError;

export function isIpcError(value: unknown): value is IpcError {
  if (typeof value !== 'object' || value === null) return false;
  const v = value as { kind?: unknown; message?: unknown };
  return typeof v.kind === 'string' && typeof v.message === 'string';
}

export function errorMessage(err: unknown): string {
  if (isIpcError(err)) {
    return `${err.kind}: ${err.message}`;
  }
  if (err instanceof Error) return err.message;
  if (typeof err === 'string') return err;
  return String(err);
}
