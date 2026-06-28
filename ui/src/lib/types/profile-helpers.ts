// Helper functions and re-exports for the ts-rs-generated Profile types. The
// bare type definitions are generated; this file only carries factories +
// type-narrowing helpers.

import type { ProfileBody } from './ProfileBody';
import type { StandardProfile } from './StandardProfile';
import type { SlideshowProfile } from './SlideshowProfile';
import type { SlideshowSource } from './SlideshowSource';
import type { PerMonitorProfile } from './PerMonitorProfile';
import type { SlideshowConfig } from './SlideshowConfig';
import type { ImageOverride } from './ImageOverride';

export type { Profile } from './Profile';
export type { ProfileBody } from './ProfileBody';
export type { StandardProfile } from './StandardProfile';
export type { StandardLayer } from './StandardLayer';
export type { SlideshowProfile } from './SlideshowProfile';
export type { SlideshowSource } from './SlideshowSource';
export type { ImageSet } from './ImageSet';
export type { ImageSource } from './ImageSource';
export type { SlideshowConfig } from './SlideshowConfig';
export type { SlideshowSort } from './SlideshowSort';
export type { SlideshowStart } from './SlideshowStart';
export type { PerMonitorProfile } from './PerMonitorProfile';
export type { PerMonitorAssignment } from './PerMonitorAssignment';
export type { MonitorRef } from './MonitorRef';
export type { MonitorPlacement } from './MonitorPlacement';
export type { TopologyFingerprint } from './TopologyFingerprint';
export type { Schedule } from './Schedule';
export type { Trigger } from './Trigger';
export type { ProfileValidity } from './ProfileValidity';
export type { DisableReason } from './DisableReason';
export type { FitMode } from './FitMode';
export type { ImageOverride } from './ImageOverride';

/** A standard profile: one or more freely-placed image layers. A single image
 *  is just a one-layer Standard. */
export function isStandardBody(body: ProfileBody): body is { type: 'standard' } & StandardProfile {
  return body.type === 'standard';
}

export function isSlideshowBody(
  body: ProfileBody,
): body is { type: 'slideshow' } & SlideshowProfile {
  return body.type === 'slideshow';
}

export function isPerMonitorBody(
  body: ProfileBody,
): body is { type: 'per_monitor' } & PerMonitorProfile {
  return body.type === 'per_monitor';
}

/** Profile flavour offered by the save-as-new dialog. */
export type ProfileKind = 'standard' | 'slideshow';

/** The per-image canvas override for `path`, when one was authored. */
export function overrideFor(source: SlideshowSource, path: string): ImageOverride | null {
  return source.overrides?.[path] ?? null;
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
