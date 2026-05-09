// profiles.jsx — Profile Manager window, Save dialog, color palette, helpers
const { useState: useStateP, useEffect: useEffectP, useMemo: useMemoP, useRef: useRefP } = React;

// Curated 12-swatch palette (each is a 2-3 stop gradient, used as the profile's
// identifying chip in selector rows, manager rows, and schedule rows).
const SWATCHES = [
  { id: 'aurora',  swatch: 'linear-gradient(90deg, #2b3675, #d6677d, #fde08a)',   label: 'Aurora' },
  { id: 'olive',   swatch: 'linear-gradient(90deg, #1a2e0e, #4a6224, #97a96a)',   label: 'Olive' },
  { id: 'plasma',  swatch: 'linear-gradient(135deg, #050018, #4c1d95, #ec4899)',  label: 'Plasma' },
  { id: 'mono',    swatch: 'linear-gradient(135deg, #1a1a1a, #4a4a4a, #1a1a1a)',  label: 'Mono' },
  { id: 'cobalt',  swatch: 'linear-gradient(135deg, #0a1845, #2563eb, #06b6d4)',  label: 'Cobalt' },
  { id: 'sakura',  swatch: 'linear-gradient(135deg, #fbcfe8, #ec4899, #831843)',  label: 'Sakura' },
  { id: 'mojave',  swatch: 'linear-gradient(90deg, #c08c5a, #6b4528, #2c1d12)',   label: 'Mojave' },
  { id: 'arctic',  swatch: 'linear-gradient(180deg, #d4e8f0, #67a3c4, #1e3a5f)',  label: 'Arctic' },
  { id: 'ember',   swatch: 'linear-gradient(90deg, #1a0a0e, #d4592a, #fbbf24)',   label: 'Ember' },
  { id: 'forest',  swatch: 'linear-gradient(180deg, #0c1a0e, #1f3a26, #4a8c5e)',  label: 'Forest' },
  { id: 'paper',   swatch: 'linear-gradient(115deg, #f5f1e8, #d4cfa8, #8a8260)',  label: 'Paper' },
  { id: 'lilac',   swatch: 'linear-gradient(135deg, #1a0a3e, #6b2d80, #d4596e)',  label: 'Lilac' },
];
window.SP_SWATCHES = SWATCHES;
const swatchById = (id) => (SWATCHES.find(s => s.id === id) || SWATCHES[0]).swatch;
window.SP_swatchById = swatchById;

// Format relative recency ("just now", "2m", "3h", "2d", "3w"). Brief is fine.
function recency(ts) {
  if (!ts) return 'never';
  const s = Math.max(0, (Date.now() - ts) / 1000);
  if (s < 30) return 'just now';
  if (s < 90) return '1m ago';
  if (s < 3600) return `${Math.round(s / 60)}m ago`;
  if (s < 3600 * 24) return `${Math.round(s / 3600)}h ago`;
  if (s < 3600 * 24 * 14) return `${Math.round(s / 86400)}d ago`;
  return `${Math.round(s / (86400 * 7))}w ago`;
}
window.SP_recency = recency;

// Tiny SVG monitor-arrangement preview, used in profile-manager rows.
function MonitorMini({ topology, w = 92, h = 36, color = 'var(--text-2)' }) {
  if (!topology || topology.length === 0) {
    return <div style={{ width: w, height: h, border: '1px dashed var(--line)', borderRadius: 3 }} />;
  }
  const xs = topology.map(m => [m.x, m.x + m.w]).flat();
  const ys = topology.map(m => [m.y, m.y + m.h]).flat();
  const minX = Math.min(...xs), maxX = Math.max(...xs);
  const minY = Math.min(...ys), maxY = Math.max(...ys);
  const bbW = Math.max(1, maxX - minX), bbH = Math.max(1, maxY - minY);
  const sx = w / bbW, sy = h / bbH;
  const s = Math.min(sx, sy) * 0.92;
  const offX = (w - bbW * s) / 2 - minX * s;
  const offY = (h - bbH * s) / 2 - minY * s;
  return (
    <svg width={w} height={h} viewBox={`0 0 ${w} ${h}`} style={{ display: 'block' }}>
      {topology.map((m, i) => (
        <rect key={i}
          x={m.x * s + offX} y={m.y * s + offY}
          width={m.w * s} height={m.h * s}
          rx="1"
          fill="none" stroke={color} strokeWidth="1.1" />
      ))}
    </svg>
  );
}
window.SP_MonitorMini = MonitorMini;

// ── Save / new-profile dialog ───────────────────────────────────────────────
function SaveProfileDialog({ open, mode, defaultName, onClose, onConfirm }) {
  const [name, setName] = useStateP('');
  const [swatch, setSwatch] = useStateP('aurora');
  const [desc, setDesc] = useStateP('');
  const inputRef = useRefP(null);

  useEffectP(() => {
    if (open) {
      setName(defaultName || '');
      setSwatch(SWATCHES[Math.floor(Math.random() * SWATCHES.length)].id);
      setDesc('');
      setTimeout(() => inputRef.current?.focus(), 30);
    }
  }, [open, defaultName]);

  if (!open) return null;
  const valid = name.trim().length > 0;
  const submit = () => { if (valid) onConfirm({ name: name.trim(), swatch, description: desc.trim() }); };

  return (
    <Backdrop2 onClose={onClose}>
      <div className="panel" style={{ width: 440, padding: 22 }}
        onKeyDown={(e) => { if (e.key === 'Enter' && valid) submit(); }}>
        <div style={{ fontSize: 15, fontWeight: 600, marginBottom: 4 }}>
          {mode === 'new' ? 'New profile' : 'Save as new profile'}
        </div>
        <div style={{ fontSize: 11, color: 'var(--text-3)', marginBottom: 18 }}>
          {mode === 'new'
            ? 'Create a blank profile. You can pick an image after.'
            : 'Capture the current canvas — image, transform, monitor layout — into a new profile.'}
        </div>

        <FieldRow label="Name">
          <input ref={inputRef} className="field ui" style={{ width: '100%' }}
            value={name} onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Travel" />
        </FieldRow>

        <FieldRow label="Colour">
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(6, 1fr)', gap: 6 }}>
            {SWATCHES.map(s => (
              <button key={s.id} title={s.label} onClick={() => setSwatch(s.id)} style={{
                height: 26, borderRadius: 4, padding: 0,
                background: s.swatch,
                border: swatch === s.id ? '2px solid var(--accent)' : '1px solid var(--line)',
                outline: swatch === s.id ? '1px solid var(--bg)' : 'none',
                cursor: 'default',
              }} />
            ))}
          </div>
        </FieldRow>

        <FieldRow label="Description" optional>
          <input className="field ui" style={{ width: '100%' }}
            value={desc} onChange={(e) => setDesc(e.target.value)}
            placeholder="Optional — what it's for, when to use it" />
        </FieldRow>

        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end', marginTop: 18 }}>
          <button className="btn" onClick={onClose}>Cancel</button>
          <button className="btn primary" onClick={submit} disabled={!valid}
            style={{ opacity: valid ? 1 : 0.5 }}>
            {mode === 'new' ? 'Create' : 'Save'}
            <span className="kbd" style={{ marginLeft: 4, background: 'oklch(0 0 0 / 0.18)', borderColor: 'oklch(0 0 0 / 0.2)', color: 'oklch(0.18 0.01 250)' }}>↵</span>
          </button>
        </div>
      </div>
    </Backdrop2>
  );
}
function FieldRow({ label, optional, children }) {
  return (
    <div style={{ marginBottom: 12 }}>
      <div style={{ fontSize: 10, fontWeight: 600, letterSpacing: '0.08em', textTransform: 'uppercase', color: 'var(--text-3)', marginBottom: 6, display: 'flex', gap: 6 }}>
        {label}{optional && <span style={{ color: 'var(--text-3)', fontWeight: 400, textTransform: 'none', letterSpacing: 0 }}>· optional</span>}
      </div>
      {children}
    </div>
  );
}

// ── Confirm dialog (delete) ─────────────────────────────────────────────────
function ConfirmDialog({ open, title, body, danger, confirmLabel, onClose, onConfirm }) {
  if (!open) return null;
  return (
    <Backdrop2 onClose={onClose}>
      <div className="panel" style={{ width: 380, padding: 20 }}>
        <div style={{ fontSize: 14, fontWeight: 600, marginBottom: 6 }}>{title}</div>
        <div style={{ fontSize: 12, color: 'var(--text-2)', marginBottom: 18 }}>{body}</div>
        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
          <button className="btn" onClick={onClose}>Cancel</button>
          <button className="btn" onClick={onConfirm} style={{
            background: danger ? 'var(--danger)' : undefined,
            borderColor: danger ? 'var(--danger)' : undefined,
            color: danger ? 'white' : undefined, fontWeight: 600,
          }}>{confirmLabel || 'Confirm'}</button>
        </div>
      </div>
    </Backdrop2>
  );
}
window.SP_ConfirmDialog = ConfirmDialog;
window.SP_SaveProfileDialog = SaveProfileDialog;

// ── Profile Manager window ──────────────────────────────────────────────────
function ProfileManager({ open, profiles, activeId, onClose, onSwitch, onCreate,
                          onUpdate, onDuplicate, onDelete, onRepair, onApply }) {
  const [q, setQ] = useStateP('');
  const [selectedId, setSelectedId] = useStateP(activeId);
  const [showSave, setShowSave] = useStateP(false);
  const [confirmDel, setConfirmDel] = useStateP(null);
  const [editingName, setEditingName] = useStateP(false);
  const [colorPopover, setColorPopover] = useStateP(false);
  const searchRef = useRefP(null);

  useEffectP(() => { if (open) setTimeout(() => searchRef.current?.focus(), 60); }, [open]);
  useEffectP(() => { if (open) setSelectedId(prev => profiles.find(p => p.id === prev) ? prev : (activeId || profiles[0]?.id)); }, [open]);

  if (!open) return null;
  const filtered = profiles
    .filter(p => !q || p.name.toLowerCase().includes(q.toLowerCase()) ||
                 (p.description || '').toLowerCase().includes(q.toLowerCase()))
    .sort((a, b) => (b.lastUsedAt || 0) - (a.lastUsedAt || 0));
  const selected = profiles.find(p => p.id === selectedId) || filtered[0];

  return (
    <Backdrop2 onClose={onClose}>
      <div className="panel" style={{
        width: 'min(1100px, 94vw)', height: 'min(680px, 88vh)',
        display: 'flex', flexDirection: 'column', overflow: 'hidden',
      }}>
        {/* Header */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '12px 16px', borderBottom: '1px solid var(--line)' }}>
          <div style={{ fontSize: 14, fontWeight: 600 }}>Profiles</div>
          <span className="chip" style={{ fontSize: 10 }}>{profiles.length}</span>
          <div style={{ display: 'flex', alignItems: 'center', gap: 6, background: 'var(--bg-2)', border: '1px solid var(--line)', borderRadius: 6, height: 28, padding: '0 10px', flex: 1, maxWidth: 320 }}>
            <SearchIcon />
            <input ref={searchRef} value={q} onChange={(e) => setQ(e.target.value)}
              placeholder="Search profiles…"
              style={{ flex: 1, background: 'transparent', border: 'none', outline: 'none', color: 'var(--text)', fontSize: 12 }} />
          </div>
          <div style={{ flex: 1 }} />
          <button className="btn" onClick={() => setShowSave(true)}>
            <PlusIcon /> New profile
          </button>
          <button className="btn"><ImportIcon /> Import…</button>
          <button className="btn ghost icon" onClick={onClose}>×</button>
        </div>

        {profiles.length === 0 ? (
          <EmptyState onCreate={() => setShowSave(true)} />
        ) : (
          <div style={{ display: 'flex', flex: 1, minHeight: 0 }}>
            {/* List */}
            <div className="scroll" style={{ width: 380, borderRight: '1px solid var(--line)' }}>
              {filtered.map(p => (
                <ProfileRow key={p.id}
                  profile={p}
                  active={p.id === activeId}
                  selected={p.id === selectedId}
                  onClick={() => setSelectedId(p.id)} />
              ))}
              {filtered.length === 0 && (
                <div style={{ padding: 24, textAlign: 'center', color: 'var(--text-3)', fontSize: 12 }}>
                  No matches.
                </div>
              )}
            </div>

            {/* Detail */}
            {selected && (
              <div className="scroll" style={{ flex: 1, padding: 22 }}>
                {/* Preview */}
                <div style={{
                  background: selected.previewBg || selected.swatch && swatchById(selected.swatch) || 'var(--bg-2)',
                  borderRadius: 8, border: '1px solid var(--line)',
                  height: 220, position: 'relative', overflow: 'hidden',
                  marginBottom: 16, opacity: selected.disabled ? 0.5 : 1,
                  filter: selected.disabled ? 'grayscale(0.7)' : 'none',
                }}>
                  {/* Monitor layout overlay */}
                  <div style={{ position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                    <MonitorMini topology={selected.topology} w={420} h={150} color="oklch(1 0 0 / 0.85)" />
                  </div>
                  <div style={{ position: 'absolute', top: 10, left: 12, fontSize: 10, fontFamily: 'var(--mono)', color: 'oklch(1 0 0 / 0.85)', textShadow: '0 1px 2px oklch(0 0 0 / 0.5)' }}>
                    {selected.sourceLabel}
                  </div>
                  {selected.disabled && (
                    <div style={{
                      position: 'absolute', inset: 0,
                      display: 'flex', alignItems: 'flex-end', padding: 12,
                      background: 'linear-gradient(180deg, transparent 60%, oklch(0 0 0 / 0.55))',
                    }}>
                      <div style={{ fontSize: 12, color: 'white', display: 'flex', alignItems: 'center', gap: 8 }}>
                        <WarnIcon /> Disabled · {selected.disabledReason}
                      </div>
                    </div>
                  )}
                </div>

                {/* Name + colour */}
                <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 12 }}>
                  <div style={{ position: 'relative' }}>
                    <button onClick={() => setColorPopover(v => !v)}
                      title="Change colour"
                      style={{
                        width: 36, height: 24, borderRadius: 4,
                        background: swatchById(selected.swatch),
                        border: '1px solid var(--line)', cursor: 'default', padding: 0,
                      }} />
                    {colorPopover && (
                      <ColorPopover value={selected.swatch}
                        onPick={(s) => { onUpdate(selected.id, { swatch: s }); setColorPopover(false); }}
                        onClose={() => setColorPopover(false)} />
                    )}
                  </div>
                  {editingName ? (
                    <input className="field ui" style={{ flex: 1, fontSize: 16, height: 30 }}
                      autoFocus defaultValue={selected.name}
                      onBlur={(e) => { onUpdate(selected.id, { name: e.target.value || selected.name }); setEditingName(false); }}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') { onUpdate(selected.id, { name: e.target.value || selected.name }); setEditingName(false); }
                        if (e.key === 'Escape') setEditingName(false);
                      }} />
                  ) : (
                    <div onClick={() => setEditingName(true)} style={{
                      flex: 1, fontSize: 18, fontWeight: 600, cursor: 'text',
                      padding: '2px 4px', borderRadius: 4,
                    }}>{selected.name}</div>
                  )}
                  {activeId === selected.id && <span className="chip active">ACTIVE</span>}
                  {selected.topologyMismatch && !selected.disabled &&
                    <span className="chip" style={{ color: 'var(--warn)', borderColor: 'color-mix(in oklab, var(--warn) 40%, var(--line))' }}>
                      Different setup
                    </span>}
                </div>

                {/* Description */}
                <textarea className="field ui" style={{
                  width: '100%', minHeight: 56, padding: 10, height: 'auto',
                  resize: 'vertical', fontSize: 12, lineHeight: 1.5, marginBottom: 16,
                }}
                  placeholder="Description (optional)"
                  defaultValue={selected.description || ''}
                  onBlur={(e) => onUpdate(selected.id, { description: e.target.value })} />

                {/* Meta grid */}
                <div style={{ display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '6px 14px', fontSize: 12, marginBottom: 16 }}>
                  <span style={{ color: 'var(--text-3)' }}>Source</span>
                  <span className="mono" style={{ color: 'var(--text)' }}>
                    {selected.sourceLabel}
                    <button className="btn ghost sm" style={{ marginLeft: 8, height: 20 }}>
                      <RevealIcon /> Reveal
                    </button>
                  </span>
                  <span style={{ color: 'var(--text-3)' }}>Topology</span>
                  <span className="mono" style={{ color: 'var(--text)' }}>
                    {selected.topology?.length || 0} monitors
                    {selected.topologyMismatch && <span style={{ color: 'var(--warn)', marginLeft: 8 }}>· authored for a different setup</span>}
                  </span>
                  <span style={{ color: 'var(--text-3)' }}>Last used</span>
                  <span className="mono" style={{ color: 'var(--text)' }}>{recency(selected.lastUsedAt)}</span>
                  <span style={{ color: 'var(--text-3)' }}>Created</span>
                  <span className="mono" style={{ color: 'var(--text)' }}>{recency(selected.createdAt)}</span>
                </div>

                {/* Actions */}
                <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', borderTop: '1px solid var(--line)', paddingTop: 14 }}>
                  {selected.disabled ? (
                    <button className="btn primary" onClick={() => { onRepair(selected); onClose(); }}>
                      <WrenchIcon /> Repair
                    </button>
                  ) : (
                    <button className="btn primary" onClick={() => { onApply(selected); }} disabled={selected.id === activeId}
                      style={{ opacity: selected.id === activeId ? 0.5 : 1 }}>
                      Apply
                    </button>
                  )}
                  <button className="btn" onClick={() => onDuplicate(selected.id)}><CopyIcon /> Duplicate</button>
                  <button className="btn"><ExportIcon /> Export</button>
                  <div style={{ flex: 1 }} />
                  <button className="btn" onClick={() => setConfirmDel(selected)} style={{
                    color: 'var(--danger)', borderColor: 'color-mix(in oklab, var(--danger) 40%, var(--line))',
                  }}><TrashIcon /> Delete</button>
                </div>
              </div>
            )}
          </div>
        )}

        <SaveProfileDialog
          open={showSave}
          mode="new"
          defaultName={`untitled-${profiles.length + 1}`}
          onClose={() => setShowSave(false)}
          onConfirm={(d) => { onCreate(d); setShowSave(false); }}
        />
        <ConfirmDialog
          open={!!confirmDel}
          title={`Delete "${confirmDel?.name}"?`}
          body="This can't be undone. Schedules pointing at this profile will be flagged."
          danger confirmLabel="Delete"
          onClose={() => setConfirmDel(null)}
          onConfirm={() => { onDelete(confirmDel.id); setConfirmDel(null); }}
        />
      </div>
    </Backdrop2>
  );
}

function ProfileRow({ profile, active, selected, onClick }) {
  return (
    <button onClick={onClick} style={{
      width: '100%', display: 'flex', alignItems: 'center', gap: 12,
      padding: '10px 14px', border: 'none', borderBottom: '1px solid var(--line)',
      background: selected ? 'color-mix(in oklab, var(--accent) 12%, transparent)' : 'transparent',
      color: 'inherit', cursor: 'default', textAlign: 'left',
      opacity: profile.disabled ? 0.55 : 1,
    }}
    onMouseEnter={(e) => { if (!selected) e.currentTarget.style.background = 'var(--panel-2)'; }}
    onMouseLeave={(e) => { if (!selected) e.currentTarget.style.background = 'transparent'; }}>
      {/* Thumb */}
      <div style={{
        width: 56, height: 36, borderRadius: 4,
        background: profile.previewBg || swatchById(profile.swatch),
        border: '1px solid var(--line)', flexShrink: 0,
        filter: profile.disabled ? 'grayscale(1)' : 'none',
        position: 'relative', overflow: 'hidden',
      }}>
        <div style={{ position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center', opacity: 0.85 }}>
          <MonitorMini topology={profile.topology} w={48} h={28} color="oklch(1 0 0 / 0.7)" />
        </div>
      </div>

      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 2 }}>
          <div style={{ width: 10, height: 10, borderRadius: 2, background: swatchById(profile.swatch), flexShrink: 0, border: '1px solid var(--line)' }} />
          <span style={{ fontSize: 13, fontWeight: 500, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{profile.name}</span>
          {active && <span className="dot live" title="active" />}
        </div>
        <div className="mono" style={{ fontSize: 10, color: 'var(--text-3)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {profile.sourceLabel}
        </div>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-end', gap: 4, flexShrink: 0 }}>
        {profile.disabled
          ? <span title={profile.disabledReason} style={{ display: 'inline-flex', alignItems: 'center', gap: 4, fontSize: 10, color: 'var(--warn)' }}>
              <WarnIcon /> Disabled
            </span>
          : <span style={{ fontSize: 10, color: 'var(--text-3)' }}>{recency(profile.lastUsedAt)}</span>}
        {profile.topologyMismatch && !profile.disabled && (
          <span title="Authored for a different setup" style={{ fontSize: 9, color: 'var(--text-3)', border: '1px solid var(--line)', borderRadius: 3, padding: '0 4px' }}>
            other setup
          </span>
        )}
      </div>
    </button>
  );
}

function ColorPopover({ value, onPick, onClose }) {
  return (
    <>
      <div onClick={onClose} style={{ position: 'fixed', inset: 0, zIndex: 50 }} />
      <div className="panel" style={{
        position: 'absolute', top: 30, left: 0, padding: 8,
        display: 'grid', gridTemplateColumns: 'repeat(6, 22px)', gap: 6, zIndex: 51, width: 'auto',
      }}>
        {SWATCHES.map(s => (
          <button key={s.id} title={s.label} onClick={() => onPick(s.id)} style={{
            width: 22, height: 22, borderRadius: 3, padding: 0,
            background: s.swatch,
            border: value === s.id ? '2px solid var(--accent)' : '1px solid var(--line)',
            cursor: 'default',
          }} />
        ))}
      </div>
    </>
  );
}

function EmptyState({ onCreate }) {
  return (
    <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', flexDirection: 'column', gap: 12 }}>
      <div style={{ fontSize: 14, fontWeight: 600 }}>No profiles yet</div>
      <div style={{ fontSize: 12, color: 'var(--text-3)', maxWidth: 340, textAlign: 'center' }}>
        A profile bundles an image, the way it's cropped, and your monitor arrangement. Make one to switch in a click.
      </div>
      <button className="btn primary" onClick={onCreate}><PlusIcon /> Create your first profile</button>
    </div>
  );
}

function Backdrop2({ children, onClose }) {
  useEffectP(() => {
    const h = (e) => { if (e.key === 'Escape') onClose(); };
    window.addEventListener('keydown', h);
    return () => window.removeEventListener('keydown', h);
  }, [onClose]);
  return (
    <div onClick={onClose} style={{
      position: 'fixed', inset: 0, zIndex: 35,
      background: 'oklch(0 0 0 / 0.6)',
      display: 'flex', alignItems: 'center', justifyContent: 'center',
      animation: 'fadeIn 120ms ease',
    }}>
      <div onClick={(e) => e.stopPropagation()}>{children}</div>
    </div>
  );
}

// ── Icons (local set so this file is independent) ──────────────────────────
const SearchIcon = () => <svg width="11" height="11" viewBox="0 0 11 11"><circle cx="4.5" cy="4.5" r="2.7" fill="none" stroke="currentColor" strokeWidth="1.2"/><path d="M6.5 6.5L9 9" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round"/></svg>;
const PlusIcon = () => <svg width="11" height="11" viewBox="0 0 11 11"><path d="M5.5 2v7M2 5.5h7" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round"/></svg>;
const ImportIcon = () => <svg width="12" height="12" viewBox="0 0 12 12"><path d="M6 2v6M3.5 5.5L6 8l2.5-2.5M2 10h8" stroke="currentColor" strokeWidth="1.3" fill="none" strokeLinecap="round" strokeLinejoin="round"/></svg>;
const ExportIcon = () => <svg width="12" height="12" viewBox="0 0 12 12"><path d="M6 8V2M3.5 4.5L6 2l2.5 2.5M2 10h8" stroke="currentColor" strokeWidth="1.3" fill="none" strokeLinecap="round" strokeLinejoin="round"/></svg>;
const CopyIcon = () => <svg width="12" height="12" viewBox="0 0 12 12"><rect x="2" y="2" width="6" height="6" rx="1" fill="none" stroke="currentColor" strokeWidth="1.2"/><rect x="4" y="4" width="6" height="6" rx="1" fill="none" stroke="currentColor" strokeWidth="1.2"/></svg>;
const TrashIcon = () => <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2.5 3.5h7M4 3.5V2h4v1.5M3.5 3.5l.5 7h4l.5-7" stroke="currentColor" strokeWidth="1.2" fill="none" strokeLinecap="round" strokeLinejoin="round"/></svg>;
const WarnIcon = () => <svg width="12" height="12" viewBox="0 0 12 12"><path d="M6 1.5L11 10.5H1z" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round"/><path d="M6 5v3M6 9v0.5" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round"/></svg>;
const WrenchIcon = () => <svg width="12" height="12" viewBox="0 0 12 12"><path d="M9 2.5a2 2 0 0 0-2.6 2.6l-4.4 4.4 1.5 1.5 4.4-4.4A2 2 0 0 0 9 2.5z" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round"/></svg>;
const RevealIcon = () => <svg width="11" height="11" viewBox="0 0 12 12"><path d="M2 3h3l1 1.5h4V9H2z" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round"/></svg>;

window.SP_ProfileManager = ProfileManager;
window.SP_WarnIcon = WarnIcon;
window.SP_WrenchIcon = WrenchIcon;
