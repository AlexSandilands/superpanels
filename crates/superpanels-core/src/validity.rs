//! Profile validity (`docs/spec/09-profiles-schedules.md`).
//!
//! Disabled profiles are surfaced everywhere greyed-out; clicking opens the
//! repair flow. Validity is *derived* — never stored on disk — so it stays
//! consistent with the live monitor set and the on-disk image / folder
//! references.

use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::config::{ImageSet, PerMonitorAssignment, Profile, ProfileBody, SpanSource};
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
                SpanSource::Slideshow { images, .. } => match images {
                    ImageSet::Folder { path, .. } => {
                        let empty_or_missing = !path.exists()
                            || std::fs::read_dir(path).map_or(true, |mut it| it.next().is_none());
                        if empty_or_missing {
                            reasons
                                .push(DisableReason::FolderMissingOrEmpty { path: path.clone() });
                        }
                    }
                    ImageSet::Playlist { paths } => {
                        if paths.iter().all(|p| !p.exists()) {
                            reasons.push(DisableReason::FolderMissingOrEmpty {
                                path: paths.first().cloned().unwrap_or_default(),
                            });
                        }
                    }
                },
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
