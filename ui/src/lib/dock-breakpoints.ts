// Window width below which SourceDock stacks above MonitorGapDock instead of
// sharing the bottom row, and ModeHint hides to keep that row readable.
// MonitorGapDock's right edge sits at ~580px; SourceDock is ~350px wide
// (~660px with slideshow chrome — playback, per-image save, shuffle/settings)
// plus its 14px right margin and a little clearance.
export function dockStackBreakpoint(slideshowActive: boolean): number {
  return slideshowActive ? 1280 : 960;
}
