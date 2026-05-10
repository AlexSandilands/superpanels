// overlays.jsx — Library modal, Settings modal, Tray popover, First-run hint
const { useState: useStateO, useEffect: useEffectO, useMemo: useMemoO, useRef: useRefO } = React;

// Generate a small library of fake images (gradient swatches)
function buildLibraryImages() {
  const recipes = [
    { name: 'aurora-pano-7680.jpg', tags: ['pano', 'night'], aspect: '32:9', resW: 7680, resH: 2160, fav: true,
      bg: 'linear-gradient(90deg, #0a1240 0%, #2c3a8e 30%, #5b3680 50%, #d44a6e 75%, #f5b56a 100%)' },
    { name: 'mojave-dunes-5120.jpg', tags: ['pano', 'desert'], aspect: '32:9', resW: 5120, resH: 1440,
      bg: 'linear-gradient(90deg, #c08c5a, #6b4528 50%, #2c1d12)' },
    { name: 'cobalt-mesh-3840.jpg', tags: ['abstract'], aspect: '16:9', resW: 3840, resH: 2160,
      bg: 'linear-gradient(135deg, #0a1845, #2563eb 60%, #06b6d4)' },
    { name: 'sakura-portrait.jpg', tags: ['portrait', 'nature'], aspect: '9:16', resW: 2160, resH: 3840, fav: true,
      bg: 'linear-gradient(180deg, #fbcfe8, #ec4899 60%, #831843)' },
    { name: 'olive-grove-pano.jpg', tags: ['pano', 'nature'], aspect: '32:9', resW: 7680, resH: 2160,
      bg: 'linear-gradient(90deg, #1a2e0e, #4a6224 40%, #97a96a 70%, #d4d99e)' },
    { name: 'cyberpunk-rain-3840.jpg', tags: ['abstract', 'night'], aspect: '16:9', resW: 3840, resH: 2160,
      bg: 'linear-gradient(160deg, #050018, #4c1d95 40%, #ec4899 75%, #fbbf24)' },
    { name: 'forest-mist-5120.jpg', tags: ['pano', 'nature'], aspect: '32:9', resW: 5120, resH: 1440,
      bg: 'linear-gradient(180deg, #0c1a0e, #1f3a26 60%, #4a8c5e)' },
    { name: 'monochrome-grid.png', tags: ['abstract'], aspect: '16:9', resW: 3840, resH: 2160,
      bg: 'linear-gradient(135deg, #1a1a1a, #4a4a4a, #1a1a1a)' },
    { name: 'sunset-bay-7680.jpg', tags: ['pano', 'beach'], aspect: '32:9', resW: 7680, resH: 2160, fav: true,
      bg: 'linear-gradient(90deg, #1a0a3e, #6b2d80 35%, #d4596e 65%, #ffaa66)' },
    { name: 'arctic-blue-3840.jpg', tags: ['nature'], aspect: '16:9', resW: 3840, resH: 2160,
      bg: 'linear-gradient(180deg, #d4e8f0, #67a3c4 60%, #1e3a5f)' },
    { name: 'lava-ridge-5120.jpg', tags: ['pano', 'desert'], aspect: '32:9', resW: 5120, resH: 1440,
      bg: 'linear-gradient(90deg, #1a0a0e, #6b1d1d 40%, #d4592a 75%, #fbbf24)' },
    { name: 'paper-fold-3840.jpg', tags: ['abstract'], aspect: '16:9', resW: 3840, resH: 2160,
      bg: 'linear-gradient(115deg, #f5f1e8, #d4cfa8 50%, #8a8260)' },
  ];
  return recipes;
}

function LibraryModal({ open, onClose, onApplyImage }) {
  const [q, setQ] = useStateO('');
  const [tag, setTag] = useStateO('all');
  const [favsOnly, setFavsOnly] = useStateO(false);
  const [imgs, setImgs] = useStateO(buildLibraryImages());
  const searchRef = useRefO(null);
  useEffectO(() => { if (open && searchRef.current) searchRef.current.focus(); }, [open]);
  const allTags = ['all', 'pano', 'nature', 'abstract', 'night', 'desert', 'beach', 'portrait'];
  const filtered = imgs.filter(i => {
    if (favsOnly && !i.fav) return false;
    if (tag !== 'all' && !i.tags.includes(tag)) return false;
    if (q && !i.name.toLowerCase().includes(q.toLowerCase())) return false;
    return true;
  });
  const toggleFav = (name) => setImgs(prev => prev.map(i => i.name === name ? { ...i, fav: !i.fav } : i));

  if (!open) return null;
  return (
    <Backdrop onClose={onClose}>
      <div className="panel" style={{
        width: 'min(1100px, 92vw)', height: 'min(720px, 88vh)',
        display: 'flex', flexDirection: 'column', overflow: 'hidden',
      }}>
        {/* Toolbar */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '12px 16px', borderBottom: '1px solid var(--line)' }}>
          <div style={{ fontSize: 14, fontWeight: 600 }}>Library</div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 6, background: 'var(--bg-2)', border: '1px solid var(--line)', borderRadius: 6, height: 28, padding: '0 10px', flex: 1, maxWidth: 360 }}>
            <span style={{ color: 'var(--text-3)' }}><IconSearchSm /></span>
            <input ref={searchRef} value={q} onChange={(e) => setQ(e.target.value)}
              placeholder="Search images, tags…"
              style={{ flex: 1, background: 'transparent', border: 'none', outline: 'none', color: 'var(--text)', fontSize: 12 }} />
            <span className="kbd">⌘L</span>
          </div>
          <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
            {allTags.map(t => (
              <button key={t} onClick={() => setTag(t)} className={tag === t ? 'chip active' : 'chip'} style={{ cursor: 'default' }}>
                {t}
              </button>
            ))}
            <button onClick={() => setFavsOnly(v => !v)} className={favsOnly ? 'chip active' : 'chip'} style={{ cursor: 'default' }}>
              <IconStarSm filled={favsOnly} /> favourites
            </button>
          </div>
          <div style={{ flex: 1 }} />
          <button className="btn ghost icon" onClick={onClose}>×</button>
        </div>

        <div style={{ display: 'flex', flex: 1, minHeight: 0 }}>
          {/* Side: roots */}
          <div style={{ width: 220, padding: 14, borderRight: '1px solid var(--line)', overflow: 'auto' }}>
            <div style={{ fontSize: 9, fontWeight: 600, letterSpacing: '0.08em', color: 'var(--text-3)', textTransform: 'uppercase', marginBottom: 8 }}>Roots</div>
            <RootRow path="~/Pictures/Wallpapers" count={847} active />
            <RootRow path="~/Downloads/panos" count={42} />
            <RootRow path="/mnt/nas/photos" count={12480} indexing />
            <button className="btn ghost sm" style={{ width: '100%', marginTop: 8, justifyContent: 'flex-start' }}>
              <IconPlusSm /> Add root
            </button>

            <div style={{ fontSize: 9, fontWeight: 600, letterSpacing: '0.08em', color: 'var(--text-3)', textTransform: 'uppercase', marginTop: 18, marginBottom: 8 }}>Filters</div>
            <FilterRow label="Resolution" value="≥ 1920px" />
            <FilterRow label="Aspect" value="any" />

            <div style={{ marginTop: 14, padding: 10, background: 'var(--bg-2)', borderRadius: 6, border: '1px solid var(--line)' }}>
              <div style={{ fontSize: 11, fontWeight: 500, marginBottom: 4 }}>Indexing /mnt/nas/photos</div>
              <div style={{ height: 4, background: 'var(--line)', borderRadius: 2, overflow: 'hidden' }}>
                <div style={{ width: '64%', height: '100%', background: 'var(--accent)' }} />
              </div>
              <div className="mono" style={{ fontSize: 10, color: 'var(--text-3)', marginTop: 4 }}>7,994 / 12,480</div>
            </div>
          </div>

          {/* Grid */}
          <div className="scroll" style={{ flex: 1, padding: 14 }}>
            <div className="mono" style={{ fontSize: 11, color: 'var(--text-3)', marginBottom: 10 }}>
              {filtered.length} of {imgs.length} images
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(220px, 1fr))', gap: 12 }}>
              {filtered.map(img => (
                <div key={img.name} className="lib-card" style={{
                  borderRadius: 8, overflow: 'hidden', border: '1px solid var(--line)',
                  background: 'var(--panel-2)', cursor: 'default',
                }}>
                  <div style={{
                    aspectRatio: img.aspect.replace(':', '/'),
                    background: img.bg, position: 'relative',
                  }}>
                    <button onClick={() => toggleFav(img.name)} style={{
                      position: 'absolute', top: 6, right: 6,
                      width: 24, height: 24, borderRadius: 4,
                      background: 'oklch(0 0 0 / 0.4)',
                      border: 'none', color: img.fav ? 'var(--warn)' : 'oklch(1 0 0 / 0.7)',
                      cursor: 'default',
                    }}>
                      <IconStarSm filled={img.fav} />
                    </button>
                    <div style={{ position: 'absolute', bottom: 6, left: 8, fontSize: 10, fontFamily: 'var(--mono)', color: 'oklch(1 0 0 / 0.85)', textShadow: '0 1px 2px oklch(0 0 0 / 0.5)' }}>
                      {img.aspect}
                    </div>
                  </div>
                  <div style={{ padding: '8px 10px' }}>
                    <div style={{ fontSize: 11, fontWeight: 500, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{img.name}</div>
                    <div className="mono" style={{ fontSize: 10, color: 'var(--text-3)', marginTop: 2, display: 'flex', justifyContent: 'space-between' }}>
                      <span>{img.resW}×{img.resH}</span>
                      <span>{img.tags.slice(0, 2).join(' · ')}</span>
                    </div>
                    <div style={{ display: 'flex', gap: 4, marginTop: 8 }}>
                      <button className="btn primary sm" style={{ flex: 1, fontSize: 10 }}
                        onClick={() => { onApplyImage(img); onClose(); }}>Apply</button>
                      <button className="btn sm icon" title="Set for monitor…"><IconLink /></button>
                      <button className="btn sm icon" title="Reveal"><IconReveal /></button>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </Backdrop>
  );
}

function RootRow({ path, count, active, indexing }) {
  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 8,
      padding: '6px 8px', borderRadius: 4,
      background: active ? 'color-mix(in oklab, var(--accent) 12%, transparent)' : 'transparent',
      marginBottom: 2,
    }}>
      <IconFolder color={active ? 'var(--accent)' : 'var(--text-3)'} />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div className="mono" style={{ fontSize: 11, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', color: active ? 'var(--accent)' : 'var(--text-2)' }}>{path}</div>
        <div style={{ fontSize: 10, color: 'var(--text-3)', marginTop: 1 }}>
          {indexing ? <span style={{ color: 'var(--warn)' }}>● indexing</span> : `${count.toLocaleString()} images`}
        </div>
      </div>
    </div>
  );
}
function FilterRow({ label, value }) {
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', padding: '4px 8px', fontSize: 11 }}>
      <span style={{ color: 'var(--text-3)' }}>{label}</span>
      <span className="mono" style={{ color: 'var(--text-2)' }}>{value}</span>
    </div>
  );
}

function SettingsModal({ open, onClose, settings, setSettings, accentOptions, accent, setAccent, theme, setTheme }) {
  const [section, setSection] = useStateO('general');
  if (!open) return null;
  const sections = [
    { id: 'general', label: 'General' },
    { id: 'appearance', label: 'Appearance' },
    { id: 'monitors', label: 'Monitors' },
    { id: 'library', label: 'Library' },
    { id: 'backends', label: 'Backends' },
    { id: 'schedules', label: 'Schedules' },
    { id: 'shortcuts', label: 'Shortcuts' },
    { id: 'about', label: 'About' },
  ];
  return (
    <Backdrop onClose={onClose}>
      <div className="panel" style={{
        width: 'min(880px, 92vw)', height: 'min(620px, 84vh)',
        display: 'flex', overflow: 'hidden',
      }}>
        <div style={{ width: 180, borderRight: '1px solid var(--line)', padding: 8, background: 'var(--panel-2)' }}>
          <div style={{ padding: '8px 10px 12px', fontSize: 13, fontWeight: 600 }}>Settings</div>
          {sections.map(s => (
            <button key={s.id} onClick={() => setSection(s.id)} style={{
              display: 'block', width: '100%', textAlign: 'left',
              padding: '7px 10px', borderRadius: 5, fontSize: 12,
              background: section === s.id ? 'color-mix(in oklab, var(--accent) 16%, transparent)' : 'transparent',
              color: section === s.id ? 'var(--accent)' : 'var(--text-2)',
              border: 'none', fontFamily: 'inherit', cursor: 'default',
              fontWeight: section === s.id ? 600 : 400,
            }}>{s.label}</button>
          ))}
        </div>
        <div className="scroll" style={{ flex: 1, padding: 28, position: 'relative' }}>
          <button className="btn ghost icon" style={{ position: 'absolute', top: 14, right: 14 }} onClick={onClose}>×</button>
          {section === 'general' && <GeneralSettings settings={settings} setSettings={setSettings} />}
          {section === 'appearance' && <AppearanceSettings accentOptions={accentOptions} accent={accent} setAccent={setAccent} theme={theme} setTheme={setTheme} />}
          {section === 'monitors' && <MonitorsSettings />}
          {section === 'library' && <LibrarySettings />}
          {section === 'backends' && <BackendsSettings />}
          {section === 'schedules' && <SchedulesSettings />}
          {section === 'shortcuts' && <ShortcutsSettings />}
          {section === 'about' && <AboutSettings />}
        </div>
      </div>
    </Backdrop>
  );
}

function SettingsHeader({ title, sub }) {
  return (
    <div style={{ marginBottom: 24 }}>
      <div style={{ fontSize: 18, fontWeight: 600 }}>{title}</div>
      {sub && <div style={{ fontSize: 12, color: 'var(--text-3)', marginTop: 4 }}>{sub}</div>}
    </div>
  );
}
function SettingRow({ label, sub, children }) {
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '12px 0', borderBottom: '1px solid var(--line)', gap: 16 }}>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 13, fontWeight: 500 }}>{label}</div>
        {sub && <div style={{ fontSize: 11, color: 'var(--text-3)', marginTop: 2 }}>{sub}</div>}
      </div>
      <div>{children}</div>
    </div>
  );
}
function Toggle({ value, onChange }) {
  return (
    <button onClick={() => onChange(!value)} style={{
      width: 36, height: 20, borderRadius: 10,
      background: value ? 'var(--accent)' : 'var(--line-2)',
      border: 'none', position: 'relative', cursor: 'default', padding: 0,
      transition: 'background 120ms',
    }}>
      <span style={{
        position: 'absolute', top: 2, left: value ? 18 : 2,
        width: 16, height: 16, borderRadius: '50%', background: 'white',
        transition: 'left 120ms',
      }} />
    </button>
  );
}
function Select({ value, onChange, options }) {
  return (
    <select value={value} onChange={(e) => onChange(e.target.value)} className="field ui" style={{ height: 28, paddingRight: 28 }}>
      {options.map(o => <option key={o.value || o} value={o.value || o}>{o.label || o}</option>)}
    </select>
  );
}

function GeneralSettings({ settings, setSettings }) {
  return (
    <>
      <SettingsHeader title="General" sub="App behaviour and notifications." />
      <SettingRow label="Autostart on login" sub="Adds an XDG autostart entry under ~/.config/autostart">
        <Toggle value={settings.autostart} onChange={(v) => setSettings(s => ({ ...s, autostart: v }))} />
      </SettingRow>
      <SettingRow label="Run in tray when window closes">
        <Toggle value={settings.trayRun} onChange={(v) => setSettings(s => ({ ...s, trayRun: v }))} />
      </SettingRow>
      <SettingRow label="Notifications" sub="Errors are always logged regardless">
        <Select value={settings.notify} onChange={(v) => setSettings(s => ({ ...s, notify: v }))}
          options={['off', 'errors only', 'all']} />
      </SettingRow>
      <SettingRow label="Reduced motion" sub="Disables canvas + apply animations. Mirrors prefers-reduced-motion.">
        <Select value={settings.motion} onChange={(v) => setSettings(s => ({ ...s, motion: v }))}
          options={['system', 'on', 'off']} />
      </SettingRow>
      <SettingRow label="Locale" sub="UI language (only English ships in v1)">
        <Select value={settings.locale} onChange={(v) => setSettings(s => ({ ...s, locale: v }))}
          options={['en-US (system)', 'en-GB', 'de-DE', 'fr-FR']} />
      </SettingRow>

      <div style={{ marginTop: 24, display: 'flex', gap: 8, flexWrap: 'wrap' }}>
        <button className="btn">Open log file</button>
        <button className="btn">Open config directory</button>
        <button className="btn">Open library DB</button>
      </div>
    </>
  );
}

function AppearanceSettings({ accentOptions, accent, setAccent, theme, setTheme }) {
  return (
    <>
      <SettingsHeader title="Appearance" />
      <SettingRow label="Theme">
        <div style={{ display: 'inline-flex', borderRadius: 6, overflow: 'hidden', border: '1px solid var(--line)' }}>
          {['auto', 'light', 'dark'].map(t => (
            <button key={t} onClick={() => setTheme(t)} style={{
              border: 'none', height: 28, padding: '0 14px',
              background: theme === t ? 'var(--accent)' : 'transparent',
              color: theme === t ? 'oklch(0.16 0.01 250)' : 'var(--text-2)',
              fontSize: 12, fontFamily: 'inherit', textTransform: 'capitalize', cursor: 'default',
            }}>{t}</button>
          ))}
        </div>
      </SettingRow>
      <SettingRow label="Accent" sub="Used for selection, primary action, dimension callouts. Follows system on KDE Plasma 6.">
        <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
          {accentOptions.map(c => (
            <button key={c} onClick={() => setAccent(c)} style={{
              width: 24, height: 24, borderRadius: '50%',
              background: c, border: accent === c ? '2px solid white' : '2px solid transparent',
              boxShadow: accent === c ? `0 0 0 2px ${c}` : 'none',
              cursor: 'default',
            }} />
          ))}
        </div>
      </SettingRow>
      <SettingRow label="Follow KDE system accent">
        <Toggle value={false} onChange={() => {}} />
      </SettingRow>
      <SettingRow label="Window blur" sub="Disable on low-power machines">
        <Toggle value={true} onChange={() => {}} />
      </SettingRow>
    </>
  );
}

function MonitorsSettings() {
  return (
    <>
      <SettingsHeader title="Monitors" sub="Detected displays and their physical sizes." />
      {[{ name: 'DP-1', model: 'LG 27UL850', wPx: 3840, hPx: 2160, wMm: 597.7, hMm: 336.2, hz: 60, src: 'compositor' },
        { name: 'DP-2', model: 'LG 27UL850', wPx: 3840, hPx: 2160, wMm: 597.7, hMm: 336.2, hz: 144, src: 'compositor', primary: true },
        { name: 'DP-3', model: 'Dell U2723QE', wPx: 3840, hPx: 2160, wMm: 597.7, hMm: 336.2, hz: 60, src: 'manual' }].map(m => (
        <div key={m.name} style={{ padding: '14px 0', borderBottom: '1px solid var(--line)', display: 'flex', alignItems: 'center', gap: 14 }}>
          <div style={{ width: 60, height: 36, border: '1.5px solid var(--line-2)', borderRadius: 3, position: 'relative' }}>
            <div style={{ position: 'absolute', inset: 3, background: 'var(--bg-2)', borderRadius: 1 }} />
          </div>
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 13, fontWeight: 600, display: 'flex', gap: 8 }}>
              {m.name}
              {m.primary && <span className="chip active">PRIMARY</span>}
            </div>
            <div className="mono" style={{ fontSize: 11, color: 'var(--text-3)', marginTop: 2 }}>
              {m.model} · {m.wPx}×{m.hPx} @ {m.hz}Hz · {m.wMm}×{m.hMm}mm
            </div>
            <div style={{ fontSize: 10, color: m.src === 'manual' ? 'var(--warn)' : 'var(--text-3)', marginTop: 4 }}>
              size from {m.src}
            </div>
          </div>
          <button className="btn sm">Edit size…</button>
        </div>
      ))}
      <div style={{ display: 'flex', gap: 8, marginTop: 16 }}>
        <button className="btn"><IconRefresh /> Re-detect (F5)</button>
        <button className="btn">Pin layout override…</button>
      </div>
    </>
  );
}

function LibrarySettings() {
  return (
    <>
      <SettingsHeader title="Library" />
      <SettingRow label="Thumbnail size">
        <Select value="medium" onChange={() => {}} options={['small', 'medium', 'large']} />
      </SettingRow>
      <SettingRow label="Auto-scan on changes" sub="Watch library roots with inotify">
        <Toggle value={true} onChange={() => {}} />
      </SettingRow>
      <SettingRow label="Cache thumbnails on disk">
        <Toggle value={true} onChange={() => {}} />
      </SettingRow>
      <SettingsHeader title="Roots" sub="Folders Superpanels indexes for the library." />
      <RootRow path="~/Pictures/Wallpapers" count={847} active />
      <RootRow path="~/Downloads/panos" count={42} />
      <RootRow path="/mnt/nas/photos" count={12480} indexing />
    </>
  );
}
function BackendsSettings() {
  const backends = [
    { name: 'KDE Plasma (plasma-apply-wallpaperimage)', avail: true, why: 'plasmashell PID 4218 detected', active: true },
    { name: 'GNOME (gsettings)', avail: false, why: 'org.gnome.Shell not on session bus' },
    { name: 'Sway / Hyprland (swaybg)', avail: false, why: 'SWAYSOCK / HYPRLAND_INSTANCE_SIGNATURE not set' },
    { name: 'wlroots generic', avail: true, why: 'wayland socket present' },
    { name: 'feh (X11 fallback)', avail: false, why: 'XDG_SESSION_TYPE=wayland' },
    { name: 'Custom shell command', avail: true, why: 'always available' },
  ];
  return (
    <>
      <SettingsHeader title="Backends" sub="The mechanism Superpanels uses to actually push the wallpaper to your compositor." />
      {backends.map(b => (
        <div key={b.name} style={{ padding: '12px 0', borderBottom: '1px solid var(--line)', display: 'flex', alignItems: 'center', gap: 12 }}>
          <span className={'dot ' + (b.avail ? (b.active ? 'live' : 'ok') : '')}
            style={{ background: b.avail ? undefined : 'var(--text-3)' }} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 13, fontWeight: 500, display: 'flex', gap: 8 }}>
              {b.name}
              {b.active && <span className="chip active">ACTIVE</span>}
            </div>
            <div className="mono" style={{ fontSize: 10, color: 'var(--text-3)', marginTop: 2 }}>
              {b.why}
            </div>
          </div>
          {b.avail && !b.active && <button className="btn sm">Pin</button>}
        </div>
      ))}
    </>
  );
}
function SchedulesSettings() {
  const [paused, setPaused] = useStateO(window.SP_schedulesPaused?.() || false);
  const [loc, setLoc] = useStateO({ lat: '52.520', lon: '13.405' });
  const [rules, setRules] = useStateO(window.SP_getSchedules?.() || []);
  const [editing, setEditing] = useStateO(null); // null | { id?, ... } draft
  const profiles = window.SP_getProfiles?.() || [];

  const saveRules = (next) => { setRules(next); window.SP_setSchedules?.(next); };
  const togglePaused = (v) => { setPaused(v); window.SP_setSchedulesPaused?.(v); };

  const conflictWith = (rule) => {
    const minute = (r) => r.kind === 'daily' ? `${r.h}:${r.m}` : r.kind === 'sun' ? `sun:${r.event}:${r.offsetMin}` : `cron:${r.cron}`;
    const k = minute(rule);
    return rules.find(r => r.id !== rule.id && r.enabled && minute(r) === k);
  };

  return (
    <>
      <SettingsHeader title="Schedules" sub="Time-of-day triggers that switch the active profile." />

      <SettingRow label="Pause all schedules" sub="While paused, schedules don’t fire. Manual switching still works.">
        <Toggle value={paused} onChange={togglePaused} />
      </SettingRow>

      <SettingRow label="Location (lat, lon)" sub="Used for sunset / sunrise rules.">
        <div style={{ display: 'flex', gap: 6 }}>
          <input className="field mono" style={{ width: 84 }} value={loc.lat} onChange={(e) => setLoc(l => ({ ...l, lat: e.target.value }))} />
          <input className="field mono" style={{ width: 84 }} value={loc.lon} onChange={(e) => setLoc(l => ({ ...l, lon: e.target.value }))} />
        </div>
      </SettingRow>

      <div style={{ marginTop: 16, marginBottom: 8, fontSize: 11, fontWeight: 600, letterSpacing: '0.06em', color: 'var(--text-3)', textTransform: 'uppercase' }}>Rules</div>
      {rules.length === 0 && (
        <div style={{ fontSize: 12, color: 'var(--text-3)', padding: '12px 0' }}>No schedules. Add one below.</div>
      )}
      {rules.map(r => (
        <ScheduleRowFull key={r.id}
          rule={r}
          profiles={profiles}
          onToggle={(v) => saveRules(rules.map(x => x.id === r.id ? { ...x, enabled: v } : x))}
          onEdit={() => setEditing(r)}
          onDelete={() => saveRules(rules.filter(x => x.id !== r.id))}
        />
      ))}

      <button className="btn" style={{ marginTop: 16 }}
        onClick={() => setEditing({ id: null, kind: 'daily', h: 9, m: 0, event: 'sunset', offsetMin: 0, cron: '0 9 * * *', target: profiles[0]?.id, name: '', enabled: true })}>
        <IconPlusSm /> Add schedule
      </button>

      {editing && (
        <ScheduleEditor
          draft={editing}
          profiles={profiles}
          conflict={conflictWith}
          onCancel={() => setEditing(null)}
          onSave={(d) => {
            const next = d.id ? rules.map(x => x.id === d.id ? d : x) : [...rules, { ...d, id: 's' + Math.random().toString(36).slice(2, 8) }];
            saveRules(next); setEditing(null);
          }}
        />
      )}
    </>
  );
}

function ScheduleRowFull({ rule, profiles, onToggle, onEdit, onDelete }) {
  const target = profiles.find(p => p.id === rule.target);
  const summary = rule.kind === 'daily' ? `Daily at ${String(rule.h).padStart(2,'0')}:${String(rule.m).padStart(2,'0')}`
    : rule.kind === 'sun' ? `${rule.event === 'sunset' ? 'Sunset' : 'Sunrise'} ${rule.offsetMin >= 0 ? '+' : ''}${rule.offsetMin} min`
    : `cron: ${rule.cron}`;
  const next = rule.enabled ? (rule.kind === 'daily' ? `${String(rule.h).padStart(2,'0')}:${String(rule.m).padStart(2,'0')}` : '—') : 'paused';
  return (
    <div style={{ padding: '12px 0', borderBottom: '1px solid var(--line)', display: 'flex', alignItems: 'center', gap: 14 }}>
      <Toggle value={rule.enabled} onChange={onToggle} />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 13, fontWeight: 500 }}>{rule.name || summary}</div>
        <div className="mono" style={{ fontSize: 11, color: 'var(--text-3)', marginTop: 2, display: 'flex', gap: 8, alignItems: 'center' }}>
          <span>{summary}</span>
          <span>·</span>
          <span style={{ display: 'inline-flex', alignItems: 'center', gap: 5 }}>
            {target && <span style={{ width: 10, height: 7, borderRadius: 1, background: window.SP_swatchById(target.swatchId), border: '1px solid var(--line)' }} />}
            <span style={{ color: target ? 'var(--text-2)' : 'var(--danger)' }}>{target ? target.name : 'missing target'}</span>
          </span>
          <span>·</span>
          <span>next {next}</span>
        </div>
      </div>
      <button className="btn sm" onClick={onEdit}>Edit</button>
      <button className="btn sm icon" title="Delete" onClick={onDelete} style={{ color: 'var(--danger)' }}>×</button>
    </div>
  );
}

function ScheduleEditor({ draft, profiles, conflict, onCancel, onSave }) {
  const [d, setD] = useStateO(draft);
  const conf = conflict({ ...d });
  return (
    <div className="panel" style={{ marginTop: 14, padding: 14, borderRadius: 8 }}>
      <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 10 }}>{d.id ? 'Edit schedule' : 'New schedule'}</div>

      <div style={{ display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '10px 14px', alignItems: 'center', marginBottom: 12 }}>
        <span style={{ fontSize: 11, color: 'var(--text-3)' }}>Trigger</span>
        <div style={{ display: 'inline-flex', borderRadius: 6, overflow: 'hidden', border: '1px solid var(--line)', alignSelf: 'start' }}>
          {[['daily', 'Daily'], ['sun', 'Sunset/Sunrise'], ['cron', 'Cron']].map(([k, l]) => (
            <button key={k} onClick={() => setD({ ...d, kind: k })} style={{
              border: 'none', height: 26, padding: '0 12px', fontSize: 11, fontFamily: 'inherit',
              background: d.kind === k ? 'var(--accent)' : 'transparent',
              color: d.kind === k ? 'oklch(0.16 0.01 250)' : 'var(--text-2)', cursor: 'default',
            }}>{l}</button>
          ))}
        </div>

        {d.kind === 'daily' && (
          <>
            <span style={{ fontSize: 11, color: 'var(--text-3)' }}>Time</span>
            <div style={{ display: 'inline-flex', gap: 6, alignItems: 'center' }}>
              <input className="field mono" type="number" min="0" max="23" style={{ width: 56 }} value={d.h} onChange={(e) => setD({ ...d, h: Math.max(0, Math.min(23, parseInt(e.target.value) || 0)) })} />
              <span>:</span>
              <input className="field mono" type="number" min="0" max="59" style={{ width: 56 }} value={d.m} onChange={(e) => setD({ ...d, m: Math.max(0, Math.min(59, parseInt(e.target.value) || 0)) })} />
            </div>
          </>
        )}
        {d.kind === 'sun' && (
          <>
            <span style={{ fontSize: 11, color: 'var(--text-3)' }}>Event</span>
            <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
              <select className="field ui" value={d.event} onChange={(e) => setD({ ...d, event: e.target.value })}>
                <option value="sunset">Sunset</option>
                <option value="sunrise">Sunrise</option>
              </select>
              <input className="field mono" type="number" style={{ width: 80 }} value={d.offsetMin} onChange={(e) => setD({ ...d, offsetMin: parseInt(e.target.value) || 0 })} />
              <span style={{ fontSize: 11, color: 'var(--text-3)' }}>min offset</span>
            </div>
          </>
        )}
        {d.kind === 'cron' && (
          <>
            <span style={{ fontSize: 11, color: 'var(--text-3)' }}>Expression</span>
            <input className="field mono" style={{ width: '100%' }} value={d.cron} onChange={(e) => setD({ ...d, cron: e.target.value })} />
          </>
        )}

        <span style={{ fontSize: 11, color: 'var(--text-3)' }}>Target</span>
        <select className="field ui" value={d.target} onChange={(e) => setD({ ...d, target: e.target.value })}>
          {profiles.map(p => (
            <option key={p.id} value={p.id} disabled={p.disabled}>
              {p.name}{p.disabled ? ' — disabled' : ''}
            </option>
          ))}
        </select>

        <span style={{ fontSize: 11, color: 'var(--text-3)' }}>Name</span>
        <input className="field ui" placeholder="optional" value={d.name} onChange={(e) => setD({ ...d, name: e.target.value })} />
      </div>

      {conf && (
        <div style={{
          display: 'flex', gap: 8, alignItems: 'center',
          padding: '8px 10px', borderRadius: 6, fontSize: 11,
          background: 'color-mix(in oklab, var(--danger) 16%, var(--panel))',
          border: '1px solid color-mix(in oklab, var(--danger) 40%, var(--line))',
          color: 'var(--text)', marginBottom: 12,
        }}>
          <span style={{ color: 'var(--danger)' }}>⚠</span>
          Conflicts with another rule that fires at the same minute: <span style={{ fontWeight: 600 }}>{conf.name || 'rule ' + conf.id}</span>
        </div>
      )}

      <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
        <button className="btn" onClick={onCancel}>Cancel</button>
        <button className="btn primary" disabled={!!conf} onClick={() => onSave(d)} style={{ opacity: conf ? 0.5 : 1 }}>Save</button>
      </div>
    </div>
  );
}
function ShortcutsSettings() {
  const shortcuts = [
    ['Apply', '↵'], ['New profile', '⌘N'], ['Save profile', '⌘S'], ['Save as…', '⌘⇧S'],
    ['Switch to profile 1/2/3', '⌘1 / ⌘2 / ⌘3'],
    ['Pause / resume slideshow', 'Space'], ['Next / previous', '→ / ←'],
    ['Reset image transform', 'R'], ['Toggle off-monitor dim', 'D'],
    ['Re-detect monitors', 'F5'], ['Settings', '⌘,'], ['Focus library search', '⌘L'],
    ['Rotate selected monitor', '[ / ]'], ['Nudge monitor', '←↑→↓'], ['Big nudge', '⇧ + arrow'],
    ['Close modal', 'Esc'],
  ];
  return (
    <>
      <SettingsHeader title="Keyboard shortcuts" />
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '4px 32px' }}>
        {shortcuts.map(([l, k]) => (
          <div key={l} style={{ display: 'flex', justifyContent: 'space-between', padding: '6px 0', borderBottom: '1px solid var(--line)' }}>
            <span style={{ fontSize: 12 }}>{l}</span>
            <span className="kbd">{k}</span>
          </div>
        ))}
      </div>
    </>
  );
}
function AboutSettings() {
  return (
    <>
      <SettingsHeader title="About Superpanels" />
      <div style={{ display: 'flex', alignItems: 'center', gap: 14, padding: 16, background: 'var(--bg-2)', borderRadius: 8, border: '1px solid var(--line)' }}>
        <SuperpanelsLogoLg />
        <div>
          <div style={{ fontSize: 18, fontWeight: 600 }}>Superpanels 1.0.0</div>
          <div className="mono" style={{ fontSize: 11, color: 'var(--text-3)', marginTop: 4 }}>
            tauri 2.4.1 · svelte 5 · rust 1.83 · linux-x86_64
          </div>
          <div style={{ fontSize: 11, color: 'var(--text-3)', marginTop: 6 }}>
            Bezel-aware multi-monitor wallpapers for Linux.
          </div>
        </div>
      </div>
    </>
  );
}

function TrayPopover({ open, onClose, profiles, active, onSwitch, onSlideshow }) {
  if (!open) return null;
  return (
    <>
      <div onClick={onClose} style={{ position: 'fixed', inset: 0, zIndex: 40 }} />
      <div className="panel" style={{
        position: 'fixed', top: 46, right: 18,
        width: 280, padding: 6, zIndex: 41,
      }}>
        <div style={{ padding: '10px 12px', borderBottom: '1px solid var(--line)' }}>
          <div style={{ fontSize: 11, color: 'var(--text-3)', fontWeight: 500, letterSpacing: '0.04em' }}>SUPERPANELS</div>
          <div style={{ fontSize: 13, fontWeight: 600, marginTop: 2 }}>{active.name}</div>
          <div className="mono" style={{ fontSize: 10, color: 'var(--text-3)', marginTop: 2 }}>{active.sourceLabel}</div>
        </div>
        <div style={{ padding: '6px 0' }}>
          <div style={{ padding: '4px 12px', fontSize: 9, fontWeight: 600, letterSpacing: '0.06em', color: 'var(--text-3)', textTransform: 'uppercase' }}>Profiles</div>
          {profiles.map(p => (
            <button key={p.id} onClick={() => onSwitch(p)} style={{
              width: '100%', display: 'flex', alignItems: 'center', gap: 10,
              padding: '6px 12px', border: 'none', background: 'transparent',
              color: 'inherit', cursor: 'default', textAlign: 'left',
            }}
            onMouseEnter={(e) => e.currentTarget.style.background = 'var(--panel-2)'}
            onMouseLeave={(e) => e.currentTarget.style.background = 'transparent'}>
              <span style={{ width: 14, color: 'var(--accent)', fontSize: 12 }}>{p.id === active.id ? '✓' : ''}</span>
              <span style={{ fontSize: 12, flex: 1, fontWeight: p.id === active.id ? 600 : 400 }}>{p.name}</span>
              <ProfileSwatchSm profile={p} />
            </button>
          ))}
        </div>
        <div className="divider" style={{ margin: '4px 0' }} />
        <TrayItem icon="◀" label="Previous wallpaper" />
        <TrayItem icon="⏸" label="Pause slideshow" />
        <TrayItem icon="▶" label="Next wallpaper" />
        <div className="divider" style={{ margin: '4px 0' }} />
        <TrayItem icon={null} label="Open Superpanels" />
        <TrayItem icon={null} label="Settings…" />
        <TrayItem icon={null} label="Quit" danger />
      </div>
    </>
  );
}
function TrayItem({ icon, label, danger }) {
  return (
    <button style={{
      width: '100%', display: 'flex', alignItems: 'center', gap: 10,
      padding: '6px 12px', border: 'none', background: 'transparent',
      color: danger ? 'var(--danger)' : 'inherit', cursor: 'default', textAlign: 'left', fontSize: 12,
    }}
    onMouseEnter={(e) => e.currentTarget.style.background = 'var(--panel-2)'}
    onMouseLeave={(e) => e.currentTarget.style.background = 'transparent'}>
      <span style={{ width: 14, color: 'var(--text-3)', fontSize: 11 }}>{icon}</span>
      <span>{label}</span>
    </button>
  );
}

function Backdrop({ children, onClose }) {
  useEffectO(() => {
    const h = (e) => { if (e.key === 'Escape') onClose(); };
    window.addEventListener('keydown', h);
    return () => window.removeEventListener('keydown', h);
  }, [onClose]);
  return (
    <div onClick={onClose} style={{
      position: 'fixed', inset: 0, zIndex: 30,
      background: 'oklch(0 0 0 / 0.6)',
      display: 'flex', alignItems: 'center', justifyContent: 'center',
      animation: 'fadeIn 120ms ease',
    }}>
      <div onClick={(e) => e.stopPropagation()}>{children}</div>
    </div>
  );
}

function ProfileSwatchSm({ profile }) {
  const bg = window.SP_swatchById ? window.SP_swatchById(profile.swatchId) : (profile.swatch || 'linear-gradient(90deg, #2b3675, #d6677d, #fde08a)');
  return <div style={{ width: 22, height: 14, borderRadius: 2, background: bg, border: '1px solid var(--line)' }} />;
}
function SuperpanelsLogoLg() {
  return (
    <svg width="48" height="48" viewBox="0 0 20 20">
      <rect x="1.5" y="6" width="5" height="8" rx="0.8" fill="none" stroke="var(--accent)" strokeWidth="1.4"/>
      <rect x="7.5" y="4" width="5" height="12" rx="0.8" fill="var(--accent)" opacity="0.85"/>
      <rect x="13.5" y="6" width="5" height="8" rx="0.8" fill="none" stroke="var(--accent)" strokeWidth="1.4"/>
    </svg>
  );
}

const IconSearchSm = () => <svg width="11" height="11" viewBox="0 0 11 11"><circle cx="4.5" cy="4.5" r="2.7" fill="none" stroke="currentColor" strokeWidth="1.2"/><path d="M6.5 6.5L9 9" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round"/></svg>;
const IconStarSm = ({ filled }) => <svg width="13" height="13" viewBox="0 0 13 13"><path d="M6.5 1.5l1.5 3.2 3.4.5-2.5 2.4.6 3.4-3-1.7-3 1.7.6-3.4-2.5-2.4 3.4-.5z" fill={filled ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="1" strokeLinejoin="round"/></svg>;
const IconPlusSm = () => <svg width="11" height="11" viewBox="0 0 11 11"><path d="M5.5 2v7M2 5.5h7" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round"/></svg>;
const IconLink = () => <svg width="12" height="12" viewBox="0 0 12 12"><path d="M5 3H3a2 2 0 0 0 0 4h2M7 3h2a2 2 0 0 1 0 4H7M4 5h4" stroke="currentColor" strokeWidth="1.3" fill="none" strokeLinecap="round"/></svg>;
const IconReveal = () => <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2 3h3l1 1.5h4V9H2z" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round"/></svg>;
const IconFolder = ({ color }) => <svg width="13" height="13" viewBox="0 0 13 13"><path d="M1.5 4h3l1 1h6V10h-10z" fill="none" stroke={color || 'currentColor'} strokeWidth="1.2" strokeLinejoin="round"/></svg>;
const IconRefresh = () => <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2 6a4 4 0 1 0 1.5-3.1M2 1.5V4h2.5" stroke="currentColor" strokeWidth="1.3" fill="none" strokeLinecap="round" strokeLinejoin="round"/></svg>;

window.LibraryModal = LibraryModal;
window.SettingsModal = SettingsModal;
window.TrayPopover = TrayPopover;
