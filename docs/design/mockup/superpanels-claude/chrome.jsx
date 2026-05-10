// chrome.jsx — floating chrome around the canvas: title bar, profile pill, inspector, library, settings
const { useState: useStateC, useEffect: useEffectC, useRef: useRefC, useMemo: useMemoC } = React;

function TitleBar({ activeProfile, onProfileSwitch, profiles, lastApply, backendName, onOpenLibrary, onOpenSettings, onApply, onTrayClick, onOpenProfileManager, onSaveAsNew, schedulesPaused, onTogglePauseSchedules, nextSchedule, canSaveAsNew }) {
  const [profileMenu, setProfileMenu] = useStateC(false);
  return (
    <div style={{
      position: 'absolute', top: 0, left: 0, right: 0,
      height: 40, display: 'flex', alignItems: 'center',
      padding: '0 12px', gap: 10,
      background: 'color-mix(in oklab, var(--bg) 70%, transparent)',
      borderBottom: '1px solid var(--line)',
      WebkitAppRegion: 'drag',
      zIndex: 10
    }}>
      {/* App mark */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginRight: 6 }}>
        <SuperpanelsLogo />
        <span style={{ fontWeight: 600, fontSize: 13, letterSpacing: '-0.01em' }}>Superpanels</span>
      </div>

      <div style={{ width: 1, height: 18, background: 'var(--line)' }} />

      {/* Profile pill — center-left */}
      <div style={{ position: 'relative', WebkitAppRegion: 'no-drag' }}>
        <button className="btn" style={{ height: 26, fontSize: 12 }}
        onClick={() => setProfileMenu((v) => !v)}>
          <span style={{ width: 16, height: 10, borderRadius: 2, background: window.SP_swatchById ? window.SP_swatchById(activeProfile.swatchId) : (activeProfile.swatch || 'var(--accent)'), border: '1px solid var(--line)', display: 'inline-block' }} />
          <span style={{ fontWeight: 600 }}>{activeProfile.name}</span>
          {schedulesPaused && <span className="chip" style={{ height: 16, fontSize: 9, padding: '0 6px' }}>schedules paused</span>}
          <Caret />
        </button>
        {profileMenu &&
        <TraySelector
          profiles={profiles}
          active={activeProfile}
          schedulesPaused={schedulesPaused}
          nextSchedule={nextSchedule}
          onPick={(p) => {onProfileSwitch(p);setProfileMenu(false);}}
          onOpenManager={() => { setProfileMenu(false); onOpenProfileManager(); }}
          onTogglePause={onTogglePauseSchedules}
          onClose={() => setProfileMenu(false)} />

        }
      </div>

      <div style={{ flex: 1 }} />

      {/* Status pills center-right */}
      <div style={{ display: 'flex', gap: 8, alignItems: 'center', WebkitAppRegion: 'no-drag' }}>
        <span className="chip" title="Last apply">
          <span className="dot ok" />
          <span className="mono" style={{ color: 'var(--text-2)' }}>{backendName}</span>
          <span style={{ color: 'var(--text-3)' }}>·</span>
          <span className="mono" style={{ color: 'var(--text-3)' }}>{lastApply}</span>
        </span>
        <button className="btn ghost icon" title="Library (Ctrl+L)" onClick={onOpenLibrary}>
          <IconGrid />
        </button>
        <button className="btn ghost icon" title="Settings (Ctrl+,)" onClick={onOpenSettings}>
          <IconGear />
        </button>
        <button className="btn ghost icon" title="System tray" onClick={onTrayClick}>
          <IconTray />
        </button>
        <div style={{ width: 1, height: 18, background: 'var(--line)' }} />
        <button className="btn ghost icon" title={canSaveAsNew ? 'Save current canvas as new profile' : 'No image on canvas'}
          onClick={canSaveAsNew ? onSaveAsNew : undefined}
          style={{ opacity: canSaveAsNew ? 1 : 0.4, cursor: canSaveAsNew ? 'default' : 'not-allowed' }}>
          <IconSaveNew />
        </button>
        <button className="btn ghost icon" title="Profile manager" onClick={onOpenProfileManager}>
          <IconStack />
        </button>
        <button className="btn primary" onClick={onApply} title="Apply (Enter)">
          <IconCheck /> Apply
          <span className="kbd" style={{ marginLeft: 4, background: 'oklch(0 0 0 / 0.18)', borderColor: 'oklch(0 0 0 / 0.2)', color: 'oklch(0.18 0.01 250)' }}>↵</span>
        </button>

        <div style={{ width: 1, height: 18, background: 'var(--line)' }} />

        {/* Linux-style window controls — minimize / maximize / close, right-aligned */}
        <div style={{ display: 'flex', gap: 4, paddingLeft: 2 }}>
          <WinCtl kind="min" />
          <WinCtl kind="max" />
          <WinCtl kind="close" />
        </div>
      </div>
    </div>);

}

function WinCtl({ kind }) {
  const icon = kind === 'min'
    ? <svg width="10" height="10" viewBox="0 0 10 10"><path d="M2 7h6" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round"/></svg>
    : kind === 'max'
    ? <svg width="10" height="10" viewBox="0 0 10 10"><rect x="2" y="2" width="6" height="6" fill="none" stroke="currentColor" strokeWidth="1.2" rx="0.5"/></svg>
    : <svg width="10" height="10" viewBox="0 0 10 10"><path d="M2.5 2.5l5 5M7.5 2.5l-5 5" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round"/></svg>;
  const isClose = kind === 'close';
  return (
    <button title={kind === 'min' ? 'Minimize' : kind === 'max' ? 'Maximize' : 'Close'}
      style={{
        appearance: 'none', border: '1px solid var(--line)',
        background: 'var(--panel-2)', color: 'var(--text-2)',
        width: 22, height: 22, borderRadius: '50%',
        display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
        cursor: 'default', padding: 0,
        transition: 'background 80ms, color 80ms, border-color 80ms',
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.background = isClose ? '#ec6a5e' : 'var(--line)';
        e.currentTarget.style.color = isClose ? 'white' : 'var(--text)';
        e.currentTarget.style.borderColor = isClose ? '#ec6a5e' : 'var(--line-2)';
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.background = 'var(--panel-2)';
        e.currentTarget.style.color = 'var(--text-2)';
        e.currentTarget.style.borderColor = 'var(--line)';
      }}
    >{icon}</button>
  );
}

function TraySelector({ profiles, active, schedulesPaused, nextSchedule, onPick, onOpenManager, onTogglePause, onClose }) {
  const [q, setQ] = useStateC('');
  const inputRef = useRefC(null);
  useEffectC(() => {
    const h = (e) => { if (e.key === 'Escape') onClose(); };
    window.addEventListener('keydown', h);
    setTimeout(() => inputRef.current?.focus(), 30);
    return () => window.removeEventListener('keydown', h);
  }, [onClose]);

  const filtered = profiles
    .filter(p => !q || p.name.toLowerCase().includes(q.toLowerCase()))
    .sort((a, b) => (b.lastUsedAt || 0) - (a.lastUsedAt || 0));

  return (
    <>
      <div onClick={onClose} style={{ position: 'fixed', inset: 0, zIndex: 20 }} />
      <div className="panel" style={{ position: 'absolute', top: 32, left: 0, width: 320, padding: 0, zIndex: 21, overflow: 'hidden' }}>
        {/* Search */}
        {profiles.length >= 4 && (
          <div style={{ padding: '8px 10px', borderBottom: '1px solid var(--line)', display: 'flex', alignItems: 'center', gap: 6, background: 'var(--bg-2)' }}>
            <span style={{ color: 'var(--text-3)' }}><IconSearch /></span>
            <input ref={inputRef} value={q} onChange={(e) => setQ(e.target.value)}
              placeholder="Search profiles…"
              style={{ flex: 1, background: 'transparent', border: 'none', outline: 'none', color: 'var(--text)', fontSize: 12 }} />
          </div>
        )}

        {/* Schedule hint */}
        {nextSchedule && !schedulesPaused && (
          <div style={{ padding: '8px 12px', fontSize: 11, color: 'var(--text-3)', borderBottom: '1px solid var(--line)', background: 'color-mix(in oklab, var(--accent) 6%, transparent)' }}>
            <span style={{ color: 'var(--accent)' }}>●</span> Auto: switching to <span style={{ color: 'var(--text)', fontWeight: 500 }}>{nextSchedule.targetName}</span> at <span className="mono">{nextSchedule.atHHMM}</span>
          </div>
        )}

        {/* Profile rows */}
        <div className="scroll" style={{ maxHeight: 340, padding: 4 }}>
          {profiles.length === 0 ? (
            <div style={{ padding: '20px 14px', textAlign: 'center', fontSize: 12, color: 'var(--text-2)' }}>
              <div style={{ marginBottom: 8 }}>No profiles yet</div>
              <button className="btn primary sm" onClick={onOpenManager}>Create one in the manager</button>
            </div>
          ) : filtered.length === 0 ? (
            <div style={{ padding: '14px', textAlign: 'center', fontSize: 11, color: 'var(--text-3)' }}>No matches</div>
          ) : filtered.map(p => (
            <button key={p.id} onClick={() => onPick(p)} style={{
              width: '100%', display: 'flex', alignItems: 'center', gap: 10,
              padding: '7px 10px', borderRadius: 6, border: 'none',
              background: p.id === active.id ? 'color-mix(in oklab, var(--accent) 14%, transparent)' : 'transparent',
              color: 'inherit', cursor: 'default', textAlign: 'left',
              opacity: p.disabled ? 0.55 : 1, marginBottom: 1,
            }}
            title={p.disabled ? `Disabled · ${p.disabledReason} — click to repair` : p.name}
            onMouseEnter={(e) => { if (p.id !== active.id) e.currentTarget.style.background = 'var(--panel-2)'; }}
            onMouseLeave={(e) => { if (p.id !== active.id) e.currentTarget.style.background = 'transparent'; }}>
              <span style={{ width: 18, height: 12, borderRadius: 2,
                background: window.SP_swatchById(p.swatchId), border: '1px solid var(--line)',
                flexShrink: 0, filter: p.disabled ? 'grayscale(1)' : 'none' }} />
              <span style={{ flex: 1, minWidth: 0, fontSize: 12, fontWeight: p.id === active.id ? 600 : 500,
                overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{p.name}</span>
              {p.disabled && <span title={p.disabledReason} style={{ color: 'var(--warn)', display: 'inline-flex' }}><svg width="11" height="11" viewBox="0 0 12 12"><path d="M6 1.5L11 10.5H1z" fill="none" stroke="currentColor" strokeWidth="1.2"/><path d="M6 5v3M6 9v0.3" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round"/></svg></span>}
              {p.id === active.id && <span className="dot live" title="active" />}
            </button>
          ))}
        </div>

        {/* Footer items */}
        <div style={{ borderTop: '1px solid var(--line)', padding: 4 }}>
          <button onClick={onOpenManager} style={menuItem()}>
            <IconStack /> Open profile manager…
          </button>
          <button onClick={onTogglePause} style={menuItem()}>
            {schedulesPaused
              ? <><svg width="11" height="11" viewBox="0 0 12 12"><path d="M3 2v8l7-4z" fill="currentColor"/></svg> Resume schedules</>
              : <><svg width="11" height="11" viewBox="0 0 12 12"><rect x="3" y="2" width="2.4" height="8" fill="currentColor"/><rect x="6.6" y="2" width="2.4" height="8" fill="currentColor"/></svg> Pause schedules</>}
          </button>
        </div>
      </div>
    </>
  );
}

function menuItem() {
  return {
    width: '100%', display: 'flex', alignItems: 'center', gap: 8,
    padding: '7px 10px', borderRadius: 5, border: 'none',
    background: 'transparent', color: 'var(--text)', cursor: 'default',
    textAlign: 'left', fontSize: 12, fontFamily: 'inherit',
  };
}

function ProfileMenu({ profiles, active, onPick, onClose }) {
  useEffectC(() => {
    const h = (e) => {if (e.key === 'Escape') onClose();};
    window.addEventListener('keydown', h);
    return () => window.removeEventListener('keydown', h);
  }, [onClose]);
  return (
    <>
      <div onClick={onClose} style={{ position: 'fixed', inset: 0, zIndex: 20 }} />
      <div className="panel" style={{
        position: 'absolute', top: 32, left: 0, width: 280,
        padding: 6, zIndex: 21
      }}>
        <div style={{ padding: '6px 8px', fontSize: 10, fontWeight: 600, letterSpacing: '0.06em', color: 'var(--text-3)', textTransform: 'uppercase' }}>Profiles</div>
        {profiles.map((p, i) =>
        <button key={p.id} onClick={() => onPick(p)} style={{
          width: '100%', display: 'flex', alignItems: 'center', gap: 10,
          padding: '8px 10px', borderRadius: 6,
          border: 'none', background: p.id === active.id ? 'color-mix(in oklab, var(--accent) 15%, transparent)' : 'transparent',
          color: 'inherit', cursor: 'default', textAlign: 'left'
        }}
        onMouseEnter={(e) => {if (p.id !== active.id) e.currentTarget.style.background = 'var(--panel-2)';}}
        onMouseLeave={(e) => {if (p.id !== active.id) e.currentTarget.style.background = 'transparent';}}>

            <ProfileSwatch profile={p} />
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontWeight: p.id === active.id ? 600 : 500, fontSize: 12, display: 'flex', gap: 6, alignItems: 'center' }}>
                {p.name}
                {p.default && <span style={{ fontSize: 9, color: 'var(--text-3)', border: '1px solid var(--line)', padding: '0 4px', borderRadius: 3 }}>DEFAULT</span>}
              </div>
              <div className="mono" style={{ fontSize: 10, color: 'var(--text-3)', marginTop: 1 }}>
                {p.mode} · {p.sourceLabel}
              </div>
            </div>
            {i < 3 && <span className="kbd">⌘{i + 1}</span>}
          </button>
        )}
        <div className="divider" />
        <button className="btn ghost" style={{ width: '100%', justifyContent: 'flex-start' }}>
          <IconPlus /> New profile <span className="kbd" style={{ marginLeft: 'auto' }}>⌘N</span>
        </button>
        <button className="btn ghost" style={{ width: '100%', justifyContent: 'flex-start' }}>
          <IconSave /> Save current as new <span className="kbd" style={{ marginLeft: 'auto' }}>⌘⇧S</span>
        </button>
      </div>
    </>);

}

function ProfileSwatch({ profile }) {
  return (
    <div style={{
      width: 28, height: 18, borderRadius: 3,
      background: profile.swatch || 'linear-gradient(90deg, #2b3675, #d6677d, #fde08a)',
      border: '1px solid var(--line)',
      flexShrink: 0
    }} />);

}

function ToolDock({ mode, onMode, fitMode, setFitMode, dimOff, setDimOff, zoom, setZoom, onResetTransform, onSnapCover, onApplyLayoutReset, density }) {
  const compact = density === 'compact';
  return (
    <div className="panel" style={{
      position: 'absolute', left: 14, top: 56,
      padding: 6, display: 'flex', flexDirection: 'column', gap: 4,
      width: 44,
      zIndex: 5
    }}>
      <ToolBtn icon={<IconMove />} label="Move (auto)" active />
      <ToolBtn icon={<IconCover />} label="Snap to cover" onClick={onSnapCover} />
      <ToolBtn icon={<IconReset />} label="Reset image transform (R)" onClick={onResetTransform} />
      <ToolBtn icon={<IconLayout />} label="Reset monitor layout" onClick={onApplyLayoutReset} />
      <div style={{ height: 1, background: 'var(--line)', margin: '4px 0' }} />
      <ToolBtn icon={<IconDim />} label="Off-monitor dim (D)" active={dimOff} onClick={() => setDimOff(!dimOff)} />
      <div style={{ height: 1, background: 'var(--line)', margin: '4px 0' }} />
      <button className="btn ghost icon sm" onClick={() => setZoom((z) => Math.min(2.0, z + 0.1))} title="Zoom in"><IconPlus /></button>
      <div className="mono" style={{ textAlign: 'center', fontSize: 9, color: 'var(--text-3)' }}>{Math.round(zoom * 100)}%</div>
      <button className="btn ghost icon sm" onClick={() => setZoom((z) => Math.max(0.5, z - 0.1))} title="Zoom out"><IconMinus /></button>
      <button className="btn ghost icon sm" onClick={() => setZoom(1)} title="Fit"><IconFit /></button>
    </div>);

}

function ToolBtn({ icon, label, active, onClick }) {
  return (
    <button
      className="btn ghost icon"
      title={label}
      onClick={onClick}
      style={{
        width: 32, height: 32,
        background: active ? 'color-mix(in oklab, var(--accent) 16%, transparent)' : 'transparent',
        color: active ? 'var(--accent)' : 'var(--text-2)'
      }}>

      {icon}
    </button>);

}

// Bezel inspector — bottom-left
function BezelDock({ bezelMm, setBezelMm, fitMode, setFitMode, layoutSizeMm, monitorCount, totalPx, density }) {
  const [collapsed, setCollapsed] = useStateC(false);
  if (collapsed) {
    return (
      <CollapsedTab side="left" left={70} bottom={14} label="Bezels"
        summary={`${bezelMm.h.toFixed(1)} · ${bezelMm.v.toFixed(1)} mm`}
        onExpand={() => setCollapsed(false)} />
    );
  }
  return (
    <div className="panel" style={{
      position: 'absolute', left: 70, bottom: 14,
      padding: 12, paddingRight: 28, display: 'flex', gap: 18, alignItems: 'center',
      zIndex: 5
    }}>
      <div>
        <div style={{ fontSize: 9, fontWeight: 600, letterSpacing: '0.08em', color: 'var(--text-3)', textTransform: 'uppercase', marginBottom: 6 }}>Bezel gap</div>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <BezelInput label="H" value={bezelMm.h} onChange={(v) => setBezelMm((b) => ({ ...b, h: v }))} />
          <BezelInput label="V" value={bezelMm.v} onChange={(v) => setBezelMm((b) => ({ ...b, v: v }))} />
        </div>
      </div>
      <div style={{ width: 1, height: 36, background: 'var(--line)' }} />
      <div>
        <div style={{ fontSize: 9, fontWeight: 600, letterSpacing: '0.08em', color: 'var(--text-3)', textTransform: 'uppercase', marginBottom: 6 }}>Fit</div>
        <div style={{ display: 'inline-flex', borderRadius: 6, overflow: 'hidden', border: '1px solid var(--line)' }}>
          {['Fill', 'Fit', 'Stretch', 'Center'].map((f) =>
          <button key={f} onClick={() => setFitMode(f)} style={{
            border: 'none', height: 26, padding: '0 10px',
            fontSize: 11, fontWeight: 500, fontFamily: 'inherit',
            background: fitMode === f ? 'var(--accent)' : 'transparent',
            color: fitMode === f ? 'oklch(0.16 0.01 250)' : 'var(--text-2)',
            cursor: 'default'
          }}>{f}</button>
          )}
        </div>
      </div>
      <div style={{ width: 1, height: 36, background: 'var(--line)' }} />
      <div>
        <div style={{ fontSize: 9, fontWeight: 600, letterSpacing: '0.08em', color: 'var(--text-3)', textTransform: 'uppercase', marginBottom: 6 }}>Layout</div>
        <div className="mono" style={{ fontSize: 11, color: 'var(--text-2)' }}>
          {monitorCount} mons · <span style={{ color: 'var(--text)' }}>{Math.round(layoutSizeMm.w)}×{Math.round(layoutSizeMm.h)}</span> mm · <span style={{ color: 'var(--text-3)' }}>{totalPx.w}×{totalPx.h} px</span>
        </div>
      </div>
      <CollapseChevron side="left-collapse" onClick={() => setCollapsed(true)} title="Collapse" />
    </div>);

}

// Collapse handle attached to the inner edge of a dock
function CollapseChevron({ side, onClick, title }) {
  const isLeftDock = side === 'left-collapse'; // bezel dock — chevron points left to collapse toward left edge
  const right = isLeftDock ? -10 : 'auto';
  const left = isLeftDock ? 'auto' : -10;
  const path = isLeftDock ? 'M5 2L2 5l3 3' : 'M3 2l3 3-3 3';
  return (
    <button onClick={onClick} title={title}
      style={{
        position: 'absolute', top: '50%', right, left, transform: 'translateY(-50%)',
        width: 20, height: 28, borderRadius: 4,
        background: 'var(--panel-2)', border: '1px solid var(--line)',
        color: 'var(--text-3)', cursor: 'default',
        display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
        padding: 0, transition: 'background 80ms, color 80ms',
      }}
      onMouseEnter={(e) => { e.currentTarget.style.background = 'var(--line)'; e.currentTarget.style.color = 'var(--text)'; }}
      onMouseLeave={(e) => { e.currentTarget.style.background = 'var(--panel-2)'; e.currentTarget.style.color = 'var(--text-3)'; }}
    >
      <svg width="8" height="10" viewBox="0 0 8 10"><path d={path} stroke="currentColor" strokeWidth="1.4" fill="none" strokeLinecap="round" strokeLinejoin="round"/></svg>
    </button>
  );
}

// Tab shown when a dock is collapsed — click to expand
function CollapsedTab({ side, left, right, bottom, label, summary, onExpand }) {
  const isLeft = side === 'left';
  const path = isLeft ? 'M3 2l3 3-3 3' : 'M5 2L2 5l3 3';
  return (
    <button onClick={onExpand} title={`Expand ${label}`}
      className="panel"
      style={{
        position: 'absolute', left, right, bottom,
        height: 32, padding: '0 10px 0 12px', borderRadius: 16,
        display: 'inline-flex', alignItems: 'center', gap: 8,
        cursor: 'default', zIndex: 5,
        font: 'inherit',
      }}
    >
      <span style={{ fontSize: 10, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: 'var(--text-3)' }}>{label}</span>
      <span className="mono" style={{ fontSize: 11, color: 'var(--text-2)' }}>{summary}</span>
      <svg width="9" height="10" viewBox="0 0 8 10" style={{ color: 'var(--text-3)' }}>
        <path d={path} stroke="currentColor" strokeWidth="1.4" fill="none" strokeLinecap="round" strokeLinejoin="round"/>
      </svg>
    </button>
  );
}

function BezelInput({ label, value, onChange }) {
  const [hover, setHover] = useStateC(false);
  const inputRef = useRefC(null);
  const step = (delta, big) => onChange(Math.max(0, parseFloat((value + delta * (big ? 5 : 0.5)).toFixed(2))));
  const onWheel = (e) => {
    e.preventDefault();
    step(e.deltaY < 0 ? 1 : -1, e.shiftKey);
  };
  const onKey = (e) => {
    if (e.key === 'ArrowUp') { e.preventDefault(); step(1, e.shiftKey); }
    if (e.key === 'ArrowDown') { e.preventDefault(); step(-1, e.shiftKey); }
  };
  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      onWheel={onWheel}
      title="Scroll, ↑/↓, or use the steppers (Shift = ×10)"
      style={{
        display: 'inline-flex', alignItems: 'stretch',
        height: 26, borderRadius: 6,
        border: `1px solid ${hover ? 'var(--line-2)' : 'var(--line)'}`,
        background: 'var(--bg-2)', overflow: 'hidden',
        transition: 'border-color 80ms',
      }}
    >
      <span className="mono" style={{
        display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
        width: 18, fontSize: 10, color: 'var(--text-3)',
        borderRight: '1px solid var(--line)', background: 'color-mix(in oklab, var(--bg) 60%, transparent)',
      }}>{label}</span>
      <input
        ref={inputRef}
        type="text"
        inputMode="decimal"
        className="mono no-spin"
        value={value.toFixed(1)}
        onChange={(e) => {
          const v = parseFloat(e.target.value);
          if (!Number.isNaN(v)) onChange(v);
        }}
        onKeyDown={onKey}
        style={{
          width: 44, height: '100%', padding: '0 6px',
          background: 'transparent', border: 'none', outline: 'none',
          color: 'var(--text)', textAlign: 'right', fontSize: 12,
        }}
      />
      <span className="mono" style={{
        display: 'inline-flex', alignItems: 'center', paddingRight: 4,
        fontSize: 10, color: 'var(--text-3)',
      }}>mm</span>
      <div style={{
        display: 'flex', flexDirection: 'column',
        borderLeft: '1px solid var(--line)',
        opacity: hover ? 1 : 0.55, transition: 'opacity 100ms',
      }}>
        <Stepper dir="up" onClick={(e) => step(1, e.shiftKey)} />
        <div style={{ height: 1, background: 'var(--line)' }} />
        <Stepper dir="down" onClick={(e) => step(-1, e.shiftKey)} />
      </div>
      <span title="Scroll wheel adjusts" style={{
        display: hover ? 'inline-flex' : 'none',
        alignItems: 'center', justifyContent: 'center',
        width: 18, color: 'var(--accent)',
        borderLeft: '1px solid var(--line)',
        background: 'color-mix(in oklab, var(--accent) 10%, transparent)',
      }}>
        <svg width="10" height="14" viewBox="0 0 10 14">
          <rect x="1" y="1" width="8" height="12" rx="4" fill="none" stroke="currentColor" strokeWidth="1"/>
          <rect x="4.3" y="3.5" width="1.4" height="3" rx="0.7" fill="currentColor"/>
          <path d="M5 10v1.6M3.6 11l1.4 1.4 1.4-1.4" stroke="currentColor" strokeWidth="0.8" fill="none" strokeLinecap="round" strokeLinejoin="round" opacity="0.7"/>
        </svg>
      </span>
    </div>
  );
}

function Stepper({ dir, onClick }) {
  return (
    <button
      onClick={onClick}
      tabIndex={-1}
      style={{
        appearance: 'none', border: 'none', background: 'transparent',
        width: 16, flex: 1, padding: 0, cursor: 'default',
        display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
        color: 'var(--text-3)', transition: 'background 60ms, color 60ms',
      }}
      onMouseEnter={(e) => { e.currentTarget.style.background = 'var(--line)'; e.currentTarget.style.color = 'var(--text)'; }}
      onMouseLeave={(e) => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.color = 'var(--text-3)'; }}
    >
      <svg width="7" height="5" viewBox="0 0 7 5">
        <path d={dir === 'up' ? 'M1 4l2.5-3 2.5 3' : 'M1 1l2.5 3 2.5-3'}
          stroke="currentColor" strokeWidth="1.2" fill="none" strokeLinecap="round" strokeLinejoin="round"/>
      </svg>
    </button>
  );
}

// Source dock — bottom-right: shows current source + slideshow controls
function SourceDock({ source, slideshow, setSlideshow, onPickFile, onOpenLibrary }) {
  const [collapsed, setCollapsed] = useStateC(false);
  if (collapsed) {
    return (
      <CollapsedTab side="right" right={14} bottom={14} label="Source"
        summary={`${source.name} · ${slideshow.index + 1}/${slideshow.total}`}
        onExpand={() => setCollapsed(false)} />
    );
  }
  return (
    <div className="panel" style={{
      position: 'absolute', right: 14, bottom: 14,
      padding: 8, paddingLeft: 28, display: 'flex', gap: 10, alignItems: 'center',
      zIndex: 5
    }}>
      <CollapseChevron side="right-collapse" onClick={() => setCollapsed(true)} title="Collapse" />
      <div style={{
        width: 56, height: 32, borderRadius: 4,
        background: source.thumbBg,
        border: '1px solid var(--line)',
        flexShrink: 0
      }} />
      <div style={{ minWidth: 140, maxWidth: 200 }}>
        <div style={{ fontSize: 11, fontWeight: 600, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {source.name}
        </div>
        <div className="mono" style={{ fontSize: 10, color: 'var(--text-3)', marginTop: 2 }}>
          {source.dims} · {source.sizeKb}
        </div>
      </div>
      <div style={{ width: 1, height: 28, background: 'var(--line)' }} />
      <div style={{ display: 'flex', gap: 2, alignItems: 'center' }}>
        <button className="btn ghost icon sm" title="Previous (←)" onClick={() => setSlideshow((s) => ({ ...s, index: Math.max(0, s.index - 1) }))}><IconPrev /></button>
        <button className="btn ghost icon sm" title="Pause/resume (Space)"
        onClick={() => setSlideshow((s) => ({ ...s, paused: !s.paused }))}>
          {slideshow.paused ? <IconPlay /> : <IconPause />}
        </button>
        <button className="btn ghost icon sm" title="Next (→)" onClick={() => setSlideshow((s) => ({ ...s, index: s.index + 1 }))}><IconNext /></button>
      </div>
      <div className="mono" style={{ fontSize: 10, color: 'var(--text-3)' }}>
        {slideshow.index + 1}/{slideshow.total}
      </div>
      <div style={{ width: 1, height: 28, background: 'var(--line)' }} />
      <button className="btn sm" onClick={onOpenLibrary}><IconGrid /> Library</button>
    </div>);

}

// Inspector — shows when a monitor is selected (right side)
function MonitorInspector({ monitor, onClose, onUpdate, onSetPrimary, onRotate, allMonitors, bezelMm }) {
  if (!monitor) return null;
  const r = window.SP_monRect(monitor);
  return (
    <div className="panel scroll" style={{
      position: 'absolute', right: 14, top: 56,
      width: 300, maxHeight: 'calc(100vh - 200px)',
      padding: 14,
      zIndex: 6
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 12 }}>
        <span className="dot live" />
        <div style={{ fontSize: 13, fontWeight: 600 }}>{monitor.name}</div>
        {monitor.primary && <span className="chip active">PRIMARY</span>}
        <button className="btn ghost icon sm" onClick={onClose} style={{ marginLeft: 'auto' }} title="Close">×</button>
      </div>

      <div className="mono" style={{ fontSize: 11, color: 'var(--text-2)', marginBottom: 14 }}>
        {monitor.model}
      </div>

      <Section label="Resolution & rate">
        <Kv k="Mode" v={`${monitor.wPx}×${monitor.hPx} @ ${monitor.hz} Hz`} />
        <Kv k="Scale" v="1.00×" />
        <Kv k="Rotation" v={`${monitor.rotation}°`} />
      </Section>

      <Section label="Physical size">
        <Kv k="Width" v={`${monitor.wMm.toFixed(1)} mm`} />
        <Kv k="Height" v={`${monitor.hMm.toFixed(1)} mm`} />
        <Kv k="Diagonal" v={`${(Math.sqrt(monitor.wMm ** 2 + monitor.hMm ** 2) / 25.4).toFixed(1)}"`} />
      </Section>

      <Section label="Position (mm)">
        <div style={{ display: 'flex', gap: 6 }}>
          <NumField label="x" value={monitor.xMm} onChange={(v) => onUpdate({ xMm: v })} />
          <NumField label="y" value={monitor.yMm} onChange={(v) => onUpdate({ yMm: v })} />
        </div>
      </Section>

      <Section label="Crop on this screen">
        <div style={{ aspectRatio: monitor.wPx / monitor.hPx, background: 'var(--bg-2)', borderRadius: 4, border: '1px solid var(--line)', position: 'relative', overflow: 'hidden' }}>
          <div style={{ position: 'absolute', inset: 0, background: 'linear-gradient(90deg, #2b3675, #7d4ca0, #d6677d)' }} />
          <div style={{ position: 'absolute', bottom: 4, right: 6, fontSize: 9, fontFamily: 'var(--mono)', color: 'oklch(1 0 0 / 0.7)' }}>
            {monitor.wPx}×{monitor.hPx}
          </div>
        </div>
      </Section>

      <div style={{ display: 'flex', gap: 6, marginTop: 14 }}>
        <button className="btn sm" onClick={() => onRotate(-90)} title="[ rotate CCW">↺</button>
        <button className="btn sm" onClick={() => onRotate(90)} title="] rotate CW">↻</button>
        {!monitor.primary &&
        <button className="btn sm" onClick={onSetPrimary} style={{ marginLeft: 'auto' }}>Set as primary</button>
        }
      </div>
    </div>);

}

function Section({ label, children }) {
  return (
    <div style={{ marginBottom: 14 }}>
      <div style={{ fontSize: 9, fontWeight: 600, letterSpacing: '0.08em', color: 'var(--text-3)', textTransform: 'uppercase', marginBottom: 6 }}>{label}</div>
      {children}
    </div>);

}
function Kv({ k, v }) {
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', padding: '3px 0', fontSize: 12 }}>
      <span style={{ color: 'var(--text-3)' }}>{k}</span>
      <span className="mono" style={{ color: 'var(--text)' }}>{v}</span>
    </div>);

}
function NumField({ label, value, onChange }) {
  return (
    <div style={{ flex: 1, display: 'flex', alignItems: 'center', gap: 4 }}>
      <span className="mono" style={{ fontSize: 10, color: 'var(--text-3)' }}>{label}</span>
      <input className="field mono" style={{ flex: 1, height: 26 }}
      type="number" value={Math.round(value)}
      onChange={(e) => onChange(parseFloat(e.target.value || 0))} />
    </div>);

}

// SVG Icons
const Caret = () => <svg width="10" height="10" viewBox="0 0 10 10"><path d="M2 4l3 3 3-3" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round" /></svg>;
const IconCheck = () => <svg width="13" height="13" viewBox="0 0 13 13"><path d="M2.5 6.5l3 3 5-6" stroke="currentColor" strokeWidth="1.8" fill="none" strokeLinecap="round" strokeLinejoin="round" /></svg>;
const IconGrid = () => <svg width="14" height="14" viewBox="0 0 14 14"><rect x="2" y="2" width="4" height="4" rx="0.5" fill="none" stroke="currentColor" strokeWidth="1.3" /><rect x="8" y="2" width="4" height="4" rx="0.5" fill="none" stroke="currentColor" strokeWidth="1.3" /><rect x="2" y="8" width="4" height="4" rx="0.5" fill="none" stroke="currentColor" strokeWidth="1.3" /><rect x="8" y="8" width="4" height="4" rx="0.5" fill="none" stroke="currentColor" strokeWidth="1.3" /></svg>;
const IconGear = () => <svg width="14" height="14" viewBox="0 0 14 14"><circle cx="7" cy="7" r="2.2" fill="none" stroke="currentColor" strokeWidth="1.3" /><path d="M7 1.5v1.8M7 10.7v1.8M1.5 7h1.8M10.7 7h1.8M3.1 3.1l1.3 1.3M9.6 9.6l1.3 1.3M3.1 10.9l1.3-1.3M9.6 4.4l1.3-1.3" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" /></svg>;
const IconTray = () => <svg width="14" height="14" viewBox="0 0 14 14"><rect x="2" y="2" width="10" height="10" rx="2" fill="none" stroke="currentColor" strokeWidth="1.3" /><circle cx="5" cy="9" r="0.7" fill="currentColor" /><circle cx="7" cy="9" r="0.7" fill="currentColor" /><circle cx="9" cy="9" r="0.7" fill="currentColor" /></svg>;
const IconMove = () => <svg width="14" height="14" viewBox="0 0 14 14"><path d="M7 2v10M2 7h10M7 2l-1.6 1.6M7 2l1.6 1.6M7 12l-1.6-1.6M7 12l1.6-1.6M2 7l1.6-1.6M2 7l1.6 1.6M12 7l-1.6-1.6M12 7l-1.6 1.6" stroke="currentColor" strokeWidth="1.2" fill="none" strokeLinecap="round" /></svg>;
const IconCover = () => <svg width="14" height="14" viewBox="0 0 14 14"><rect x="1.5" y="3.5" width="11" height="7" rx="1" fill="none" stroke="currentColor" strokeWidth="1.2" /><path d="M3 5l8 4M3 9l8-4" stroke="currentColor" strokeWidth="1" opacity="0.5" /></svg>;
const IconReset = () => <svg width="14" height="14" viewBox="0 0 14 14"><path d="M2.5 7a4.5 4.5 0 1 0 1.5-3.4M3 2v2.5h2.5" stroke="currentColor" strokeWidth="1.3" fill="none" strokeLinecap="round" strokeLinejoin="round" /></svg>;
const IconLayout = () => <svg width="14" height="14" viewBox="0 0 14 14"><rect x="1" y="3" width="3.5" height="8" rx="0.5" fill="none" stroke="currentColor" strokeWidth="1.2" /><rect x="5.5" y="3" width="3" height="8" rx="0.5" fill="none" stroke="currentColor" strokeWidth="1.2" /><rect x="9.5" y="3" width="3.5" height="8" rx="0.5" fill="none" stroke="currentColor" strokeWidth="1.2" /></svg>;
const IconDim = () => <svg width="14" height="14" viewBox="0 0 14 14"><circle cx="7" cy="7" r="5" fill="none" stroke="currentColor" strokeWidth="1.3" /><path d="M7 2a5 5 0 0 1 0 10z" fill="currentColor" /></svg>;
const IconPlus = () => <svg width="12" height="12" viewBox="0 0 12 12"><path d="M6 2v8M2 6h8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" /></svg>;
const IconMinus = () => <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2 6h8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" /></svg>;
const IconFit = () => <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2 4V2h2M10 4V2H8M2 8v2h2M10 8v2H8" stroke="currentColor" strokeWidth="1.4" fill="none" strokeLinecap="round" /></svg>;
const IconSave = () => <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2 2h7l1 1v7H2V2zM4 2v3h4V2M4 7h4v3H4z" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" /></svg>;
const IconPrev = () => <svg width="12" height="12" viewBox="0 0 12 12"><path d="M8 2L4 6l4 4" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round" /></svg>;
const IconNext = () => <svg width="12" height="12" viewBox="0 0 12 12"><path d="M4 2l4 4-4 4" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round" /></svg>;
const IconPlay = () => <svg width="12" height="12" viewBox="0 0 12 12"><path d="M3 2v8l7-4z" fill="currentColor" /></svg>;
const IconPause = () => <svg width="12" height="12" viewBox="0 0 12 12"><rect x="3" y="2" width="2.4" height="8" fill="currentColor" /><rect x="6.6" y="2" width="2.4" height="8" fill="currentColor" /></svg>;
const IconSearch = () => <svg width="12" height="12" viewBox="0 0 12 12"><circle cx="5" cy="5" r="3" fill="none" stroke="currentColor" strokeWidth="1.3" /><path d="M7.2 7.2L10 10" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" /></svg>;
const IconSaveNew = () => <svg width="14" height="14" viewBox="0 0 14 14"><path d="M2 2.5h7l1.5 1.5v6.5a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round"/><path d="M4 2.5v3h4v-3" stroke="currentColor" strokeWidth="1.2" fill="none"/><circle cx="10" cy="10" r="2.6" fill="var(--panel-2)" stroke="currentColor" strokeWidth="1.2"/><path d="M10 8.7v2.6M8.7 10h2.6" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round"/></svg>;
const IconStack = () => <svg width="14" height="14" viewBox="0 0 14 14"><rect x="2" y="2.5" width="10" height="3" rx="0.6" fill="none" stroke="currentColor" strokeWidth="1.2"/><rect x="2" y="6.5" width="10" height="3" rx="0.6" fill="none" stroke="currentColor" strokeWidth="1.2"/><path d="M3 11.5h8" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round"/></svg>;
const IconStar = ({ filled }) => <svg width="12" height="12" viewBox="0 0 12 12"><path d="M6 1.5l1.4 3 3.1.4-2.3 2.2.6 3.1L6 8.6l-2.8 1.6.6-3.1L1.5 4.9l3.1-.4z" fill={filled ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="1" strokeLinejoin="round" /></svg>;

const SuperpanelsLogo = () =>
<svg width="20" height="20" viewBox="0 0 20 20">
    <rect x="1.5" y="6" width="5" height="8" rx="0.8" fill="none" stroke="var(--accent)" strokeWidth="1.4" />
    <rect x="7.5" y="4" width="5" height="12" rx="0.8" fill="var(--accent)" opacity="0.85" />
    <rect x="13.5" y="6" width="5" height="8" rx="0.8" fill="none" stroke="var(--accent)" strokeWidth="1.4" />
  </svg>;


window.TitleBar = TitleBar;
window.ToolDock = ToolDock;
window.BezelDock = BezelDock;
window.SourceDock = SourceDock;
window.MonitorInspector = MonitorInspector;
window.SP_Icons = { IconGrid, IconGear, IconStar, IconSearch, IconPlus, IconCheck };
