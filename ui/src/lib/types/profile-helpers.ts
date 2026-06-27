// Helper functions and re-exports for the ts-rs-generated Profile types
//. The bare type definitions are now
// generated; this file only carries factories + type-narrowing helpers.

import type { ProfileBody } from './ProfileBody';
import type { SpanProfile } from './SpanProfile';
import type { CompositeProfile } from './CompositeProfile';
import type { PerMonitorProfile } from './PerMonitorProfile';
import type { SlideshowConfig } from './SlideshowConfig';
import type { SpanSource } from './SpanSource';
import type { ImageSet } from './ImageSet';
import type { ImageOverride } from './ImageOverride';

export type { Profile } from './Profile';
export type { ProfileBody } from './ProfileBody';
export type { SpanProfile } from './SpanProfile';
export type { CompositeProfile } from './CompositeProfile';
export type { CompositeLayer } from './CompositeLayer';
export type { SpanSource } from './SpanSource';
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

export function isSpanBody(body: ProfileBody): body is { type: 'span' } & SpanProfile {
  return body.type === 'span';
}

export function isPerMonitorBody(
  body: ProfileBody,
): body is { type: 'per_monitor' } & PerMonitorProfile {
  return body.type === 'per_monitor';
}

export function isCompositeBody(
  body: ProfileBody,
): body is { type: 'composite' } & CompositeProfile {
  return body.type === 'composite';
}

export type SlideshowSource = {
  type: 'slideshow';
  images: ImageSet;
  config: SlideshowConfig;
  overrides?: { [key in string]: ImageOverride };
  /** One layout for every image — see `SpanSource::Slideshow::uniform_layout`. */
  uniform_layout?: boolean;
};

/** Profile flavour offered by the save-as-new dialog. */
export type ProfileKind = 'single' | 'slideshow' | 'composite';

export function isSlideshowSource(source: SpanSource): source is SlideshowSource {
  return source.type === 'slideshow';
}

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
