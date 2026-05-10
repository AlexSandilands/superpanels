// app.jsx — main App: state, shortcuts, layout, tweaks
const { useState: useStateA, useEffect: useEffectA, useRef: useRefA, useMemo: useMemoA, useCallback: useCallbackA } = React;

const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "theme": "dark",
  "accent": "#3daee9",
  "density": "regular",
  "dimsAlways": true
}/*EDITMODE-END*/;

const ACCENT_OPTIONS = ['#3daee9', '#7c5cff', '#34d399', '#ff7849', '#e8e8e8', '#f0b6c5'];

const MONITORS_3UP = [{ x: 0, y: 0, w: 600, h: 336 }, { x: 615, y: 0, w: 600, h: 336 }, { x: 1230, y: 0, w: 600, h: 336 }];
const MONITORS_2UP = [{ x: 0, y: 0, w: 600, h: 336 }, { x: 615, y: 0, w: 600, h: 336 }];
const MONITORS_PORTRAIT = [{ x: 0, y: 0, w: 336, h: 600 }, { x: 350, y: 130, w: 600, h: 336 }];

const INITIAL_PROFILES = [
  { id: 'home', name: 'Home', mode: 'span', sourceLabel: '~/Pictures/aurora-pano-7680.jpg',
    swatchId: 'aurora', description: 'Daily wallpaper for the desk.',
    topology: MONITORS_3UP, lastUsedAt: Date.now() - 1000 * 60 * 2, createdAt: Date.now() - 86400e3 * 60,
    previewBg: 'linear-gradient(90deg, #0e1a3a, #2b3675, #7d4ca0, #d6677d, #f0a05a, #fde08a)' },
  { id: 'work', name: 'Work', mode: 'span', sourceLabel: 'slideshow · ~/wallpapers/work',
    swatchId: 'olive', description: 'Calm slideshow rotation for focus blocks.',
    topology: MONITORS_3UP, lastUsedAt: Date.now() - 1000 * 60 * 60 * 5, createdAt: Date.now() - 86400e3 * 32,
    previewBg: 'linear-gradient(90deg, #1a2e0e, #4a6224, #97a96a, #d4d99e)' },
  { id: 'movie', name: 'Movie', mode: 'per-monitor', sourceLabel: '3 images',
    swatchId: 'plasma', description: 'Dim, dark, plasma. Per-monitor.',
    topology: MONITORS_3UP, lastUsedAt: Date.now() - 86400e3 * 2, createdAt: Date.now() - 86400e3 * 12,
    previewBg: 'linear-gradient(160deg, #050018, #4c1d95 40%, #ec4899 75%, #fbbf24)' },
  { id: 'focus', name: 'Focus', mode: 'span', sourceLabel: 'monochrome-grid.png',
    swatchId: 'mono', description: '',
    topology: MONITORS_3UP, lastUsedAt: Date.now() - 86400e3 * 8, createdAt: Date.now() - 86400e3 * 9,
    previewBg: 'linear-gradient(135deg, #1a1a1a, #4a4a4a, #1a1a1a)' },
  { id: 'travel', name: 'Travel laptop', mode: 'span', sourceLabel: 'arctic-blue-3840.jpg',
    swatchId: 'arctic', description: 'For when only the built-in display is connected.',
    topology: [{ x: 0, y: 0, w: 600, h: 336 }],
    lastUsedAt: Date.now() - 86400e3 * 21, createdAt: Date.now() - 86400e3 * 21,
    previewBg: 'linear-gradient(180deg, #d4e8f0, #67a3c4 60%, #1e3a5f)',
    disabled: true, disabledReason: 'authored for 1 monitor; current setup has 3', topologyMismatch: true },
  { id: 'studio', name: 'Studio (portrait)', mode: 'per-monitor', sourceLabel: 'sakura-portrait.jpg',
    swatchId: 'sakura', description: 'Tall left, wide right.',
    topology: MONITORS_PORTRAIT, lastUsedAt: Date.now() - 86400e3 * 35, createdAt: Date.now() - 86400e3 * 70,
    previewBg: 'linear-gradient(180deg, #fbcfe8, #ec4899 60%, #831843)',
    topologyMismatch: true },
  { id: 'demo', name: 'Demo room', mode: 'span', sourceLabel: '/mnt/nas/demo/loop.png',
    swatchId: 'cobalt', description: 'For client demos in the meeting room.',
    topology: MONITORS_2UP, lastUsedAt: Date.now() - 86400e3 * 90, createdAt: Date.now() - 86400e3 * 120,
    previewBg: 'linear-gradient(135deg, #0a1845, #2563eb 60%, #06b6d4)',
    disabled: true, disabledReason: 'image file missing' },
];

const INITIAL_SCHEDULES = [
  { id: 's1', kind: 'daily', h: 9,  m: 0,  target: 'work', name: 'Workday start', enabled: true },
  { id: 's2', kind: 'daily', h: 18, m: 30, target: 'home', name: 'Evening',       enabled: true },
  { id: 's3', kind: 'sun',   event: 'sunset', offsetMin: -15, target: 'movie', name: 'Sunset dim', enabled: true },
  { id: 's4', kind: 'cron',  cron: '0 0 * * 1', target: 'focus', name: 'Week start', enabled: false },
];

function App() {
  const [t, setTweak] = useTweaks(TWEAK_DEFAULTS);

  // Theme + accent applied at root
  useEffectA(() => {
    document.documentElement.dataset.theme = t.theme;
    document.documentElement.dataset.density = t.density;
    document.documentElement.style.setProperty('--accent', t.accent);
  }, [t.theme, t.density, t.accent]);

  // Monitors + image transform
  const [monitors, setMonitors] = useStateA(window.SP_DEFAULTS.DEFAULT_MONITORS);
  const [bezelMm, setBezelMm] = useStateA({ h: 12, v: 12 });
  const [selectedMonitorId, setSelectedMonitorId] = useStateA(null);
  const [hoveredMonitorId, setHoveredMonitorId] = useStateA(null);

  // Image transform: width/height in mm, offset in mm
  // Default: cover the union of monitors
  const initialTransform = useMemoA(() => {
    const bb = window.SP_bbox(monitors);
    const aspect = window.SP_DEFAULTS.PANO_W / window.SP_DEFAULTS.PANO_H;
    const w = bb.w + 60; // a touch larger than the union
    const h = w / aspect;
    return { offsetMmX: bb.x - 30, offsetMmY: bb.y + bb.h / 2 - h / 2, widthMm: w, heightMm: h };
  }, []);
  const [imageTransform, setImageTransform] = useStateA(initialTransform);

  const [fitMode, setFitMode] = useStateA('Fill');
  const [dimOff, setDimOff] = useStateA(false);
  const [zoom, setZoom] = useStateA(1);
  const [panOffset, setPanOffset] = useStateA({ x: 0, y: 0 });
  const [inspectorOpen, setInspectorOpen] = useStateA(false);

  // Panorama
  const [panoUrl, setPanoUrl] = useStateA(() => window.SP_buildPanoDataUrl());
  const [source, setSource] = useStateA({
    name: 'aurora-pano-7680.jpg',
    dims: '7680×2160',
    sizeKb: '6.4 MB',
    thumbBg: 'linear-gradient(90deg, #0e1a3a, #2b3675, #7d4ca0, #d6677d, #f0a05a, #fde08a)',
  });

  // Profiles
  const [profiles, setProfiles] = useStateA(INITIAL_PROFILES);
  const [activeProfileId, setActiveProfileId] = useStateA('home');
  const activeProfile = profiles.find(p => p.id === activeProfileId) || profiles[0];

  // Schedules
  const [schedules, setSchedules] = useStateA(INITIAL_SCHEDULES);
  const [schedulesPaused, setSchedulesPaused] = useStateA(false);

  // Profile manager + save dialog
  const [pmOpen, setPmOpen] = useStateA(false);
  const [saveDlg, setSaveDlg] = useStateA(false);

  // Bridges so settings panel + tray can read live profile/schedule state
  useEffectA(() => {
    window.SP_getProfiles = () => profiles;
    window.SP_getSchedules = () => schedules;
    window.SP_setSchedules = (s) => setSchedules(s);
    window.SP_schedulesPaused = () => schedulesPaused;
    window.SP_setSchedulesPaused = (v) => setSchedulesPaused(v);
  }, [profiles, schedules, schedulesPaused]);

  // Slideshow
  const [slideshow, setSlideshow] = useStateA({ paused: false, index: 16, total: 42, intervalMin: 30 });

  // UI overlays
  const [libraryOpen, setLibraryOpen] = useStateA(false);
  const [settingsOpen, setSettingsOpen] = useStateA(false);
  const [trayOpen, setTrayOpen] = useStateA(false);

  // Toasts
  const [toasts, setToasts] = useStateA([]);
  const pushToast = useCallbackA((toast) => {
    const id = Math.random().toString(36).slice(2);
    setToasts(ts => [...ts, { id, ...toast }]);
    setTimeout(() => setToasts(ts => ts.filter(x => x.id !== id)), toast.timeout || 4000);
  }, []);

  // Apply
  const [applyFlash, setApplyFlash] = useStateA({ key: 0, at: 0 });
  const handleApply = () => {
    setApplyFlash({ key: Math.random(), at: Date.now() });
    setTimeout(() => {
      pushToast({
        kind: 'ok',
        title: 'Applied',
        body: `KDE Plasma · 3 monitors · 184 ms`,
      });
    }, 220);
  };

  // Settings (persisted in tweaks would be heavy; keep local)
  const [settings, setSettings] = useStateA({
    autostart: true, trayRun: true, notify: 'errors only',
    motion: 'system', locale: 'en-US (system)',
  });

  // Layout statistics
  const layoutSizeMm = useMemoA(() => {
    const bb = window.SP_bbox(monitors);
    return { w: bb.w, h: bb.h };
  }, [monitors]);
  const totalPx = useMemoA(() => {
    let w = 0, h = 0;
    for (const m of monitors) {
      const rotated = m.rotation === 90 || m.rotation === 270;
      w += rotated ? m.hPx : m.wPx;
      h = Math.max(h, rotated ? m.wPx : m.hPx);
    }
    return { w, h };
  }, [monitors]);

  // Keyboard shortcuts
  useEffectA(() => {
    const onKey = (e) => {
      const isInput = ['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName);
      if (e.key === 'Enter' && !isInput && !libraryOpen && !settingsOpen) {
        handleApply();
      }
      if (e.key === 'Escape') {
        if (settingsOpen) setSettingsOpen(false);
        else if (libraryOpen) setLibraryOpen(false);
        else if (trayOpen) setTrayOpen(false);
        else setSelectedMonitorId(null);
      }
      if ((e.metaKey || e.ctrlKey) && e.key === ',') { e.preventDefault(); setSettingsOpen(true); }
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'l') { e.preventDefault(); setLibraryOpen(true); }
      if (!isInput && e.key === ' ') { e.preventDefault(); setSlideshow(s => ({ ...s, paused: !s.paused })); }
      if (!isInput && e.key === 'ArrowRight' && !libraryOpen && !settingsOpen) setSlideshow(s => ({ ...s, index: s.index + 1 }));
      if (!isInput && e.key === 'ArrowLeft' && !libraryOpen && !settingsOpen) setSlideshow(s => ({ ...s, index: Math.max(0, s.index - 1) }));
      if (!isInput && e.key.toLowerCase() === 'r' && !libraryOpen && !settingsOpen) {
        setImageTransform(initialTransform);
        pushToast({ kind: 'info', title: 'Image transform reset' });
      }
      if (!isInput && e.key.toLowerCase() === 'd' && !libraryOpen && !settingsOpen) setDimOff(v => !v);
      if (!isInput && e.key === 'F5') { e.preventDefault(); pushToast({ kind: 'ok', title: 'Re-detected 3 monitors', body: 'KDE compositor reported no changes' }); }
      if ((e.metaKey || e.ctrlKey) && /^[123]$/.test(e.key)) {
        e.preventDefault();
        const p = profiles[parseInt(e.key, 10) - 1];
        if (p) { handleProfileSwitch(p); }
      }
      if (!isInput && (e.key === '[' || e.key === ']') && selectedMonitorId) {
        const delta = e.key === ']' ? 90 : -90;
        setMonitors(ms => ms.map(m => m.id === selectedMonitorId ? { ...m, rotation: (m.rotation + delta + 360) % 360 } : m));
      }
      if (!isInput && selectedMonitorId && /^Arrow/.test(e.key) && !libraryOpen && !settingsOpen) {
        e.preventDefault();
        const step = e.shiftKey ? 10 : 1;
        const d = { ArrowUp: [0, -step], ArrowDown: [0, step], ArrowLeft: [-step, 0], ArrowRight: [step, 0] }[e.key];
        if (d) setMonitors(ms => ms.map(m => m.id === selectedMonitorId ? { ...m, xMm: m.xMm + d[0], yMm: m.yMm + d[1] } : m));
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [libraryOpen, settingsOpen, trayOpen, selectedMonitorId, initialTransform, pushToast]);

  // Snap-to-cover
  const handleSnapCover = () => {
    const bb = window.SP_bbox(monitors);
    const aspect = window.SP_DEFAULTS.PANO_W / window.SP_DEFAULTS.PANO_H;
    let w = bb.w, h = w / aspect;
    if (h < bb.h) { h = bb.h; w = h * aspect; }
    setImageTransform({ offsetMmX: bb.x + bb.w/2 - w/2, offsetMmY: bb.y + bb.h/2 - h/2, widthMm: w, heightMm: h });
    pushToast({ kind: 'info', title: 'Snapped image to cover', body: `${Math.round(w)}×${Math.round(h)} mm` });
  };
  const handleResetLayout = () => {
    setMonitors(window.SP_DEFAULTS.DEFAULT_MONITORS);
    pushToast({ kind: 'info', title: 'Monitor layout reset', body: 'reverted to compositor-reported positions' });
  };

  const handleProfileSwitch = (p) => {
    if (p.disabled) {
      pushToast({ kind: 'info', title: `Repair “${p.name}”`, body: p.disabledReason });
      setPmOpen(true);
      return;
    }
    setActiveProfileId(p.id);
    setProfiles(prev => prev.map(x => x.id === p.id ? { ...x, lastUsedAt: Date.now() } : x));
    pushToast({ kind: 'ok', title: `Switched to ${p.name}` });
  };

  const handleCreateProfile = (data) => {
    const id = 'p' + Math.random().toString(36).slice(2, 8);
    const next = {
      id, name: data.name, swatchId: data.swatch, description: data.description || '',
      mode: 'span', sourceLabel: source.name,
      topology: monitors.map(m => ({ x: m.xMm, y: m.yMm, w: m.wMm || 600, h: m.hMm || 336 })),
      previewBg: source.thumbBg, lastUsedAt: Date.now(), createdAt: Date.now(),
    };
    setProfiles(prev => [...prev, next]);
    setActiveProfileId(id);
    pushToast({ kind: 'ok', title: `Created “${data.name}”`, body: 'now active' });
  };

  const handleUpdateProfile = (id, patch) => {
    setProfiles(prev => prev.map(p => p.id === id ? { ...p, ...patch } : p));
  };
  const handleDuplicateProfile = (id) => {
    setProfiles(prev => {
      const src = prev.find(p => p.id === id); if (!src) return prev;
      const copy = { ...src, id: 'p' + Math.random().toString(36).slice(2, 8),
        name: `${src.name}-copy`, lastUsedAt: 0, createdAt: Date.now() };
      return [...prev, copy];
    });
    pushToast({ kind: 'ok', title: 'Duplicated' });
  };
  const handleDeleteProfile = (id) => {
    setProfiles(prev => prev.filter(p => p.id !== id));
    setSchedules(prev => prev.map(s => s.target === id ? { ...s, enabled: false } : s));
    if (activeProfileId === id) setActiveProfileId(profiles[0]?.id);
    pushToast({ kind: 'ok', title: 'Profile deleted' });
  };
  const handleRepairProfile = (p) => {
    setProfiles(prev => prev.map(x => x.id === p.id ? { ...x, disabled: false, disabledReason: null,
      topology: monitors.map(m => ({ x: m.xMm, y: m.yMm, w: m.wMm || 600, h: m.hMm || 336 })),
      topologyMismatch: false } : x));
    setActiveProfileId(p.id);
    setImageTransform(initialTransform);
    pushToast({ kind: 'info', title: `Repairing “${p.name}”`, body: 'reposition the image, then save' });
  };

  // Compute next-firing schedule for the tray hint
  const nextSchedule = useMemoA(() => {
    if (schedulesPaused) return null;
    const now = new Date();
    let best = null, bestDelta = Infinity;
    for (const r of schedules) {
      if (!r.enabled || r.kind !== 'daily') continue;
      const t = new Date(); t.setHours(r.h, r.m, 0, 0);
      let d = t - now; if (d < 0) d += 86400000;
      if (d < bestDelta) { bestDelta = d; best = r; }
    }
    if (!best) return null;
    const target = profiles.find(p => p.id === best.target);
    return { atHHMM: `${String(best.h).padStart(2, '0')}:${String(best.m).padStart(2, '0')}`, targetName: target?.name || best.target };
  }, [schedules, schedulesPaused, profiles]);

  const canSaveAsNew = !!source.name;

  const handleApplyImageFromLibrary = (img) => {
    setSource({
      name: img.name, dims: `${img.resW}×${img.resH}`, sizeKb: '—', thumbBg: img.bg,
    });
    pushToast({ kind: 'ok', title: 'Source updated', body: img.name });
  };

  const updateSelectedMonitor = (patch) => {
    setMonitors(ms => ms.map(m => m.id === selectedMonitorId ? { ...m, ...patch } : m));
  };
  const setSelectedAsPrimary = () => {
    setMonitors(ms => ms.map(m => ({ ...m, primary: m.id === selectedMonitorId })));
    pushToast({ kind: 'ok', title: 'Primary monitor changed' });
  };
  const rotateSelected = (delta) => {
    setMonitors(ms => ms.map(m => m.id === selectedMonitorId ? { ...m, rotation: (m.rotation + delta + 360) % 360 } : m));
  };

  const selectedMonitor = monitors.find(m => m.id === selectedMonitorId);

  // First-run-style hint when one monitor lacks physical info
  const someMissingMm = monitors.some(m => !m.wMm || !m.hMm);

  return (
    <div style={{ position: 'fixed', inset: 0, overflow: 'hidden' }}>
      {/* Canvas (full bleed) */}
      <window.MonitorPreviewCanvas
        monitors={monitors} setMonitors={setMonitors}
        bezelMm={bezelMm}
        imageTransform={imageTransform} setImageTransform={setImageTransform}
        panoUrl={panoUrl}
        panoSize={{ w: window.SP_DEFAULTS.PANO_W, h: window.SP_DEFAULTS.PANO_H }}
        selectedMonitorId={selectedMonitorId} setSelectedMonitorId={setSelectedMonitorId}
        hoveredMonitorId={hoveredMonitorId} setHoveredMonitorId={setHoveredMonitorId}
        fitMode={fitMode}
        dimOff={dimOff}
        showDimsAlways={t.dimsAlways}
        applyFlashKey={applyFlash.key}
        flashedAt={applyFlash.at}
        zoom={zoom} setZoom={setZoom}
        panOffset={panOffset} setPanOffset={setPanOffset}
        inspectorOpen={inspectorOpen} setInspectorOpen={setInspectorOpen}
        density={t.density}
      />

      {/* Mode hint banner — Unified hit-testing */}
      <ModeHint />

      {/* Title bar */}
      <window.TitleBar
        activeProfile={activeProfile}
        profiles={profiles}
        onProfileSwitch={handleProfileSwitch}
        onOpenProfileManager={() => setPmOpen(true)}
        onSaveAsNew={() => setSaveDlg(true)}
        canSaveAsNew={canSaveAsNew}
        schedulesPaused={schedulesPaused}
        onTogglePauseSchedules={() => { setSchedulesPaused(v => !v); pushToast({ kind: 'info', title: schedulesPaused ? 'Schedules resumed' : 'Schedules paused' }); }}
        nextSchedule={nextSchedule}
        backendName="KDE Plasma"
        lastApply="2s ago"
        onOpenLibrary={() => setLibraryOpen(true)}
        onOpenSettings={() => setSettingsOpen(true)}
        onApply={handleApply}
        onTrayClick={() => setTrayOpen(v => !v)}
      />

      {/* Tool dock */}
      <window.ToolDock
        fitMode={fitMode} setFitMode={setFitMode}
        dimOff={dimOff} setDimOff={setDimOff}
        zoom={zoom} setZoom={setZoom}
        onResetTransform={() => { setImageTransform(initialTransform); pushToast({ kind: 'info', title: 'Image transform reset' }); }}
        onSnapCover={handleSnapCover}
        onApplyLayoutReset={handleResetLayout}
        density={t.density}
      />

      {/* Bezel + fit + layout dock */}
      <window.BezelDock
        bezelMm={bezelMm} setBezelMm={setBezelMm}
        fitMode={fitMode} setFitMode={setFitMode}
        layoutSizeMm={layoutSizeMm}
        monitorCount={monitors.length}
        totalPx={totalPx}
        density={t.density}
      />

      {/* Source + slideshow dock */}
      <window.SourceDock
        source={source}
        slideshow={slideshow} setSlideshow={setSlideshow}
        onPickFile={() => setLibraryOpen(true)}
        onOpenLibrary={() => setLibraryOpen(true)}
      />

      {/* Inspector — when monitor selected */}
      {selectedMonitor && (
        <window.MonitorInspector
          monitor={selectedMonitor}
          allMonitors={monitors}
          bezelMm={bezelMm}
          onClose={() => setSelectedMonitorId(null)}
          onUpdate={updateSelectedMonitor}
          onSetPrimary={setSelectedAsPrimary}
          onRotate={rotateSelected}
        />
      )}

      {/* Diagnostics warning if mismatch */}
      {someMissingMm && (
        <div style={{
          position: 'absolute', left: '50%', top: 56, transform: 'translateX(-50%)',
          background: 'color-mix(in oklab, var(--warn) 18%, var(--panel))',
          border: '1px solid color-mix(in oklab, var(--warn) 50%, var(--line))',
          borderRadius: 6, padding: '6px 12px', fontSize: 12, zIndex: 4,
          display: 'flex', gap: 8, alignItems: 'center',
        }}>
          <span className="dot warn" />
          <span>1 monitor missing physical size — bezel math will be approximate.</span>
          <button className="btn sm">Fix</button>
        </div>
      )}

      {/* Toasts */}
      <div className="toast-stack">
        {toasts.map(t => (
          <div key={t.id} className="toast">
            <span className={'dot ' + (t.kind === 'ok' ? 'ok' : t.kind === 'err' ? '' : 'live')}
              style={{ marginTop: 4, background: t.kind === 'err' ? 'var(--danger)' : undefined }} />
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontSize: 12, fontWeight: 600 }}>{t.title}</div>
              {t.body && <div className="mono" style={{ fontSize: 11, color: 'var(--text-3)', marginTop: 2 }}>{t.body}</div>}
            </div>
          </div>
        ))}
      </div>

      {/* Library */}
      <window.LibraryModal
        open={libraryOpen}
        onClose={() => setLibraryOpen(false)}
        onApplyImage={handleApplyImageFromLibrary}
      />

      {/* Settings */}
      <window.SettingsModal
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        settings={settings} setSettings={setSettings}
        accentOptions={ACCENT_OPTIONS}
        accent={t.accent} setAccent={(v) => setTweak('accent', v)}
        theme={t.theme} setTheme={(v) => setTweak('theme', v)}
      />

      {/* Tray popover */}
      <window.TrayPopover
        open={trayOpen} onClose={() => setTrayOpen(false)}
        profiles={profiles} active={activeProfile}
        onSwitch={(p) => { handleProfileSwitch(p); setTrayOpen(false); }}
      />

      <window.SP_ProfileManager
        open={pmOpen}
        profiles={profiles}
        activeId={activeProfileId}
        onClose={() => setPmOpen(false)}
        onSwitch={handleProfileSwitch}
        onCreate={handleCreateProfile}
        onUpdate={handleUpdateProfile}
        onDuplicate={handleDuplicateProfile}
        onDelete={handleDeleteProfile}
        onRepair={handleRepairProfile}
        onApply={(p) => { handleProfileSwitch(p); }}
      />

      <window.SP_SaveProfileDialog
        open={saveDlg}
        mode="save"
        defaultName={`${activeProfile?.name || 'untitled'}-copy`}
        onClose={() => setSaveDlg(false)}
        onConfirm={(d) => { handleCreateProfile(d); setSaveDlg(false); }}
      />

      {/* Tweaks panel */}
      <TweaksPanel>
        <TweakSection label="Theme" />
        <TweakRadio label="Theme" value={t.theme}
          options={['dark', 'light']}
          onChange={(v) => setTweak('theme', v)} />
        <TweakColor label="Accent" value={t.accent}
          options={ACCENT_OPTIONS}
          onChange={(v) => setTweak('accent', v)} />
        <TweakSection label="Layout" />
        <TweakRadio label="Density" value={t.density}
          options={['compact', 'regular', 'spacious']}
          onChange={(v) => setTweak('density', v)} />
        <TweakSection label="Canvas" />
        <TweakToggle label="Always show bezel mm dimensions" value={t.dimsAlways}
          onChange={(v) => setTweak('dimsAlways', v)} />
      </TweaksPanel>
    </div>
  );
}

function ModeHint() {
  const [size, setSize] = useStateA({ w: window.innerWidth, h: window.innerHeight });
  useEffectA(() => {
    const onResize = () => setSize({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);

  // Hide entirely on tiny windows
  if (size.h < 520 || size.w < 720) return null;

  // Compact form when narrow — drop the secondary hints
  const compact = size.w < 1100;
  // Move it well clear of the bottom dock (~64px) and source dock when window is short
  const bottom = size.h < 700 ? 'auto' : 96;
  const top = size.h < 700 ? 56 : 'auto';

  const items = compact
    ? [['Drag image', 'pan'], ['Drag monitor', 'rearrange']]
    : [['Drag image', 'to pan'], ['Drag a monitor', 'to rearrange'], ['Scroll', 'to zoom'], ['Alt', 'disables snap']];

  return (
    <div style={{
      position: 'absolute', left: '50%', bottom, top, transform: 'translateX(-50%)',
      background: 'color-mix(in oklab, var(--panel) 85%, transparent)',
      border: '1px solid var(--line)',
      borderRadius: 16, padding: '4px 12px',
      display: 'flex', gap: 10, alignItems: 'center',
      fontSize: 11, color: 'var(--text-3)',
      backdropFilter: 'blur(12px)', WebkitBackdropFilter: 'blur(12px)',
      pointerEvents: 'none', zIndex: 4,
      whiteSpace: 'nowrap', maxWidth: 'calc(100vw - 40px)',
      overflow: 'hidden', textOverflow: 'ellipsis',
    }}>
      {items.map(([k, v], i) => (
        <React.Fragment key={i}>
          {i > 0 && <span style={{ opacity: 0.4 }}>·</span>}
          <span><span style={{ color: 'var(--text-2)' }}>{k}</span> {v}</span>
        </React.Fragment>
      ))}
    </div>
  );
}

const root = ReactDOM.createRoot(document.getElementById('root'));
root.render(<App />);
