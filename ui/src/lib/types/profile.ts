// Hand-mirrored Profile types — the Rust types live in
// `superpanels_core::config::{Profile,ProfileBody,…}`. ts-rs only exports a
// few payload types so far; until that's broadened, this file is the typed
// reflection of the Profile schema.

export type FitMode = 'fill' | 'fit' | 'stretch' | 'center';

export type Bezels = {
  horizontal_mm: number;
  vertical_mm: number;
};

export type ImageSet =
  | { type: 'folder'; path: string; recursive: boolean }
  | { type: 'playlist'; paths: string[] };

export type SlideshowSort =
  | 'shuffle'
  | 'alphabetical'
  | 'date_asc'
  | 'date_desc'
  | 'last_shown_asc';
export type SlideshowStart = 'resume' | 'new_random' | 'first';

export type SlideshowConfig = {
  interval_secs: number;
  sort: SlideshowSort;
  recent_history_size: number;
  on_start: SlideshowStart;
  pause_when_active: boolean;
  skip_on_unavailable: boolean;
};

export type SpanSource =
  | { type: 'single'; path: string }
  | { type: 'slideshow'; images: ImageSet; config: SlideshowConfig };

export type SpanProfile = {
  source: SpanSource;
  fit: FitMode;
  offset: [number, number];
  // `null` defers to FitMode (legacy behaviour); a tuple pins the GUI's free
  // transform on top of the bezel-aware canvas. See docs/spec/12-gui.md §12.3.
  image_size_px?: [number, number] | null;
};

export type MonitorRef = {
  stable_id: string;
  name: string;
};

export type PerMonitorAssignment = {
  monitor: MonitorRef;
  path: string;
};

export type PerMonitorProfile = {
  assignments: PerMonitorAssignment[];
  fit: FitMode;
};

export type ProfileBody =
  | ({ type: 'span' } & SpanProfile)
  | ({ type: 'per_monitor' } & PerMonitorProfile);

export type Schedule =
  | { type: 'daily'; hour: number; minute: number; profile: string }
  | { type: 'sunset'; offset_minutes: number; profile: string }
  | { type: 'cron'; expr: string };

export type Profile = {
  name: string;
  body: ProfileBody;
  bezels: Bezels;
  backend_override?: string;
  schedule?: Schedule;
};

export function isSpanBody(body: ProfileBody): body is { type: 'span' } & SpanProfile {
  return body.type === 'span';
}

export function isPerMonitorBody(
  body: ProfileBody,
): body is { type: 'per_monitor' } & PerMonitorProfile {
  return body.type === 'per_monitor';
}

export function defaultSlideshowConfig(): SlideshowConfig {
  return {
    interval_secs: 600,
    sort: 'shuffle',
    recent_history_size: 10,
    on_start: 'resume',
    pause_when_active: false,
    skip_on_unavailable: true,
  };
}

export function defaultBezels(): Bezels {
  return { horizontal_mm: 0, vertical_mm: 0 };
}
