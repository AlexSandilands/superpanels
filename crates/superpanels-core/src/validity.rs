//! Profile validity.
//!
//! Disabled profiles are surfaced everywhere greyed-out; clicking opens the
//! repair flow. Validity is *derived* — never stored on disk — so it stays
//! consistent with the live monitor set and the on-disk image / folder
//! references.

use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::config::{
    ImageSet, ImageSource, PerMonitorAssignment, Profile, ProfileBody, SpanSource,
};
use crate::display::{Monitor, MonitorRef};
use crate::schedule::{TopologyFingerprint, monitor_key};

/// Concrete reason a profile is disabled. The GUI lists every reason that
/// applies; the repair flow targets `TopologyMismatch` specifically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DisableReason {
    /// Connected-monitor set or rotations differ from the authored fingerprint.
    TopologyMismatch {
        authored: TopologyFingerprint,
        actual: TopologyFingerprint,
    },
    ImageMissing {
        path: PathBuf,
    },
    FolderMissingOrEmpty {
        path: PathBuf,
    },
    /// Slideshow image set has no sources at all — nothing was picked yet.
    SlideshowEmpty,
    MonitorNotConnected {
        monitor: MonitorRef,
    },
    PhysicalSizeMissing {
        stable_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../ui/src/lib/types/")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProfileValidity {
    Ok,
    Disabled { reasons: Vec<DisableReason> },
}

impl ProfileValidity {
    /// Compute validity for `profile` against the live monitor set.
    #[must_use]
    pub fn evaluate(profile: &Profile, monitors: &[Monitor]) -> Self {
        let mut reasons: Vec<DisableReason> = Vec::new();

        let actual_fp = TopologyFingerprint::from_monitors(monitors);
        if actual_fp != profile.topology {
            reasons.push(DisableReason::TopologyMismatch {
                authored: profile.topology.clone(),
                actual: actual_fp,
            });
        }

        let connected_keys: HashSet<String> = monitors.iter().map(monitor_key).collect();

        for m in monitors {
            if m.physical_size_mm.is_none() {
                reasons.push(DisableReason::PhysicalSizeMissing {
                    stable_id: monitor_key(m),
                });
            }
        }

        match &profile.body {
            ProfileBody::Span(span) => match &span.source {
                SpanSource::Single { path } => {
                    if !path.exists() {
                        reasons.push(DisableReason::ImageMissing { path: path.clone() });
                    }
                }
                SpanSource::Slideshow { images, .. } => {
                    // A mixed set is usable as long as any one source can
                    // yield an image; a vanished folder next to a healthy one
                    // is the picker's problem, not a disable reason.
                    if images.is_empty() {
                        reasons.push(DisableReason::SlideshowEmpty);
                    } else if !image_set_has_candidates(images) {
                        reasons.push(DisableReason::FolderMissingOrEmpty {
                            path: image_set_representative_path(images),
                        });
                    }
                }
            },
            ProfileBody::PerMonitor(pm) => {
                for PerMonitorAssignment { monitor, path } in &pm.assignments {
                    if !connected_keys.contains(&monitor.stable_id)
                        && !connected_keys.contains(&monitor.name)
                    {
                        reasons.push(DisableReason::MonitorNotConnected {
                            monitor: monitor.clone(),
                        });
                    }
                    if !path.exists() {
                        reasons.push(DisableReason::ImageMissing { path: path.clone() });
                    }
                }
            }
        }

        if reasons.is_empty() {
            Self::Ok
        } else {
            Self::Disabled { reasons }
        }
    }

    #[must_use]
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }
}

fn image_set_has_candidates(set: &ImageSet) -> bool {
    set.sources.iter().any(|source| match source {
        ImageSource::Image { path } => path.exists(),
        ImageSource::Folder { path, .. } => {
            path.exists() && std::fs::read_dir(path).is_ok_and(|mut it| it.next().is_some())
        }
    })
}

fn image_set_representative_path(set: &ImageSet) -> PathBuf {
    set.sources
        .first()
        .map(|source| match source {
            ImageSource::Image { path } | ImageSource::Folder { path, .. } => path.clone(),
        })
        .unwrap_or_default()
}
