// Thin wrappers around `invoke` so the UI talks to typed commands and never
// reaches into Tauri directly. Every command from SPEC §12.4 is here.

import { invoke } from '@tauri-apps/api/core';
import type { IpcError } from './types/IpcError';
import type { LibraryFilter } from './types/LibraryFilter';
import type { PreviewArgs } from './types/PreviewArgs';
import type { ProfileV2 } from './types/profile';

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
  primary: boolean;
  ppi: number | null;
};

export type Profile = ProfileV2;

export type RuntimeState = {
  version: number;
  active_profile: string | null;
  slideshow: { current_index: number | null; history_len: number; paused: boolean } | null;
  last_apply_unix_secs: number | null;
};

export type AppliedReport = {
  monitors_set?: number;
  backend?: string;
  elapsed_ms?: number;
};

async function call<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  return invoke<T>(name, args);
}

export const api = {
  detectMonitors: () => call<Monitor[]>('detect_monitors'),
  redetect: () => call<{ monitors: number }>('redetect'),
  listProfiles: () => call<Profile[]>('list_profiles'),
  applyProfile: (name: string) => call<AppliedReport>('apply_profile', { name }),
  saveProfile: (profile: ProfileV2) => call<void>('save_profile', { profile }),
  deleteProfile: (name: string) => call<void>('delete_profile', { name }),
  previewCrop: (args: PreviewArgs) => call<unknown>('preview_crop', { args }),
  libraryList: (filter: LibraryFilter) => call<unknown[]>('library_list', { filter }),
  libraryThumbnail: (path: string) =>
    call<{ data: string; mime: string }>('library_thumbnail', { path }),
  // Local-only render path used by the canvas preview for any selected /
  // dropped source file — bypasses library-roots gating (`SPEC §12.4`).
  sourceThumbnail: (path: string) =>
    call<{ data: string; mime: string }>('source_thumbnail', { path }),
  libraryTag: (path: string, tag: string, on: boolean) =>
    call<void>('library_tag', { path, tag, on }),
  slideshowNext: () => call<AppliedReport>('slideshow_next'),
  slideshowPrev: () => call<AppliedReport>('slideshow_prev'),
  slideshowPause: (paused?: boolean) =>
    call<{ paused: boolean }>('slideshow_pause', paused === undefined ? {} : { paused }),
  getConfig: () => call<unknown>('get_config'),
  saveConfig: (config: unknown) => call<void>('save_config', { config }),
  currentState: () => call<RuntimeState>('current_state'),
  setAutostart: (enabled: boolean) => call<{ enabled: boolean }>('set_autostart', { enabled }),
  getAutostart: () => call<{ enabled: boolean }>('get_autostart'),
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
