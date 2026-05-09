// Helper functions and re-exports for the ts-rs-generated Profile types
// (`docs/spec/03-core-concepts.md`). The bare type definitions are now
// generated; this file only carries factories + type-narrowing helpers.

import type { ProfileBody } from './ProfileBody';
import type { SpanProfile } from './SpanProfile';
import type { PerMonitorProfile } from './PerMonitorProfile';
import type { SlideshowConfig } from './SlideshowConfig';

export type { Profile } from './Profile';
export type { ProfileBody } from './ProfileBody';
export type { SpanProfile } from './SpanProfile';
export type { SpanSource } from './SpanSource';
export type { ImageSet } from './ImageSet';
export type { SlideshowConfig } from './SlideshowConfig';
export type { SlideshowSort } from './SlideshowSort';
export type { SlideshowStart } from './SlideshowStart';
export type { PerMonitorProfile } from './PerMonitorProfile';
export type { PerMonitorAssignment } from './PerMonitorAssignment';
export type { MonitorRef } from './MonitorRef';
export type { MonitorPlacement } from './MonitorPlacement';
export type { TopologyFingerprint } from './TopologyFingerprint';
export type { ProfileColour } from './ProfileColour';
export type { Schedule } from './Schedule';
export type { Trigger } from './Trigger';
export type { ProfileValidity } from './ProfileValidity';
export type { DisableReason } from './DisableReason';
export type { LatLong } from './LatLong';
export type { FitMode } from './FitMode';

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
