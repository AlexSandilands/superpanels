// canvas.jsx — bezel-aware monitor preview canvas with unified hit-testing
const { useState, useRef, useEffect, useCallback, useMemo } = React;

// Triple 27" landscape, 16:9, 3840x2160 each, sitting in a row
// Physical width of 27" 16:9 ≈ 597.7mm × 336.2mm
const MM27 = { wMm: 597.7, hMm: 336.2, wPx: 3840, hPx: 2160 };
const DEFAULT_MONITORS = [
  { id: 'DP-1', name: 'DP-1', model: 'LG 27UL850', xMm: 0,        yMm: 0, ...MM27, rotation: 0, primary: false, hz: 60 },
  { id: 'DP-2', name: 'DP-2', model: 'LG 27UL850', xMm: 597.7+12, yMm: 0, ...MM27, rotation: 0, primary: true,  hz: 144 },
  { id: 'DP-3', name: 'DP-3', model: 'Dell U2723QE', xMm: 2*(597.7+12), yMm: 0, ...MM27, rotation: 0, primary: false, hz: 60 },
];

// Default panorama: 7680 × 2160 (3 × 4K)
const PANO_W = 7680, PANO_H = 2160;

const fmtMm = (mm) => `${Math.round(mm)} mm`;
const fmtPx = (px) => `${Math.round(px)} px`;

function bbox(mons) {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const m of mons) {
    const r = monRect(m);
    x0 = Math.min(x0, r.x); y0 = Math.min(y0, r.y);
    x1 = Math.max(x1, r.x + r.w); y1 = Math.max(y1, r.y + r.h);
  }
  return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
}
function monRect(m) {
  const rotated = m.rotation === 90 || m.rotation === 270;
  const w = rotated ? m.hMm : m.wMm;
  const h = rotated ? m.wMm : m.hMm;
  return { x: m.xMm, y: m.yMm, w, h };
}

// Generate an abstract gradient panorama as a data URL (canvas)
function buildPanoDataUrl() {
  const c = document.createElement('canvas');
  c.width = 1920; c.height = 540; // 7680x2160 aspect
  const g = c.getContext('2d');
  // Multi-stop horizontal gradient
  const grd = g.createLinearGradient(0, 0, c.width, 0);
  grd.addColorStop(0,    '#0e1a3a');
  grd.addColorStop(0.20, '#2b3675');
  grd.addColorStop(0.42, '#7d4ca0');
  grd.addColorStop(0.60, '#d6677d');
  grd.addColorStop(0.78, '#f0a05a');
  grd.addColorStop(1,    '#fde08a');
  g.fillStyle = grd; g.fillRect(0, 0, c.width, c.height);
  // Soft "sun" + glow
  const sun = g.createRadialGradient(c.width*0.72, c.height*0.55, 6, c.width*0.72, c.height*0.55, 280);
  sun.addColorStop(0, 'rgba(255,235,180,0.95)');
  sun.addColorStop(1, 'rgba(255,235,180,0)');
  g.fillStyle = sun; g.fillRect(0, 0, c.width, c.height);
  // Mountains silhouette
  g.globalAlpha = 0.35;
  g.fillStyle = '#10122b';
  g.beginPath();
  g.moveTo(0, c.height);
  let x = 0;
  while (x < c.width) {
    const peakH = 120 + Math.sin(x*0.011)*40 + Math.cos(x*0.003)*60;
    g.lineTo(x, c.height - peakH);
    x += 60 + Math.sin(x*0.02)*30;
  }
  g.lineTo(c.width, c.height); g.closePath(); g.fill();
  g.globalAlpha = 0.5;
  g.fillStyle = '#070818';
  g.beginPath(); g.moveTo(0, c.height);
  x = 0;
  while (x < c.width) {
    const peakH = 50 + Math.sin(x*0.018+1)*25 + Math.cos(x*0.006)*30;
    g.lineTo(x, c.height - peakH);
    x += 35 + Math.sin(x*0.04)*15;
  }
  g.lineTo(c.width, c.height); g.closePath(); g.fill();
  g.globalAlpha = 1;
  // Stars/dots in the dark side
  g.fillStyle = 'rgba(255,255,255,0.7)';
  for (let i = 0; i < 80; i++) {
    const sx = Math.random() * c.width * 0.5;
    const sy = Math.random() * c.height * 0.6;
    g.fillRect(sx, sy, 1, 1);
  }
  return c.toDataURL('image/jpeg', 0.85);
}

function MonitorPreviewCanvas({
  monitors, setMonitors,
  bezelMm,
  imageTransform, setImageTransform,
  panoUrl,
  panoSize,
  selectedMonitorId, setSelectedMonitorId,
  hoveredMonitorId, setHoveredMonitorId,
  fitMode,
  dimOff,
  showDimsAlways,
  applyFlashKey,
  flashedAt,
  zoom, setZoom,
  panOffset, setPanOffset,
  inspectorOpen, setInspectorOpen,
  density,
}) {
  const stageRef = useRef(null);
  const [stageSize, setStageSize] = useState({ w: 1200, h: 700 });
  const [drag, setDrag] = useState(null); // { kind: 'image'|'monitor', startX, startY, ... }
  const [tipPos, setTipPos] = useState(null);
  const [guides, setGuides] = useState([]); // alignment guides while dragging monitor

  // measure stage
  useEffect(() => {
    if (!stageRef.current) return;
    const ro = new ResizeObserver(([e]) => {
      setStageSize({ w: e.contentRect.width, h: e.contentRect.height });
    });
    ro.observe(stageRef.current);
    return () => ro.disconnect();
  }, []);

  // Compute a fit scale: physical mm space → screen px
  const layout = useMemo(() => {
    const bb = bbox(monitors);
    const pad = 100; // mm of breathing room around layout
    const totalW = bb.w + pad * 2;
    const totalH = bb.h + pad * 2;
    const sx = stageSize.w / totalW;
    const sy = stageSize.h / totalH;
    const baseScale = Math.min(sx, sy) * 0.86;
    const scale = baseScale * zoom; // mm→px
    const cx = stageSize.w / 2 + panOffset.x;
    const cy = stageSize.h / 2 + panOffset.y;
    const ox = cx - (bb.x + bb.w / 2) * scale;
    const oy = cy - (bb.y + bb.h / 2) * scale;
    return { bb, scale, ox, oy };
  }, [monitors, stageSize, zoom, panOffset]);

  const mm2px = (mmX, mmY) => ({
    x: layout.ox + mmX * layout.scale,
    y: layout.oy + mmY * layout.scale,
  });
  const px2mm = (px, py) => ({
    mmX: (px - layout.ox) / layout.scale,
    mmY: (py - layout.oy) / layout.scale,
  });

  // Compute the panorama image rect on the stage (in screen px)
  // imageTransform is { offsetMmX, offsetMmY, widthMm, heightMm } where w/h are size in mm
  const imgRect = useMemo(() => {
    const x = imageTransform.offsetMmX;
    const y = imageTransform.offsetMmY;
    const w = imageTransform.widthMm;
    const h = imageTransform.heightMm;
    const a = mm2px(x, y);
    return { x: a.x, y: a.y, w: w * layout.scale, h: h * layout.scale };
  }, [imageTransform, layout]);

  // Hit-test logic — unified canvas
  const hitTest = (clientX, clientY) => {
    const rect = stageRef.current.getBoundingClientRect();
    const px = clientX - rect.left;
    const py = clientY - rect.top;
    // Top-most monitor first (last drawn on top)
    for (let i = monitors.length - 1; i >= 0; i--) {
      const m = monitors[i];
      const r = monRect(m);
      const a = mm2px(r.x, r.y);
      const b = mm2px(r.x + r.w, r.y + r.h);
      if (px >= a.x && px <= b.x && py >= a.y && py <= b.y) {
        // Check corner / rotate handles for selected monitor
        if (selectedMonitorId === m.id) {
          const handleRadius = 10;
          const corners = [
            { name: 'rotate', x: b.x - 4, y: a.y - 18 },
          ];
          for (const c of corners) {
            if (Math.hypot(c.x - px, c.y - py) < handleRadius) return { type: 'handle', monitorId: m.id, handle: c.name };
          }
        }
        return { type: 'monitor', monitorId: m.id };
      }
    }
    // Otherwise, image (it's underneath)
    if (px >= imgRect.x && px <= imgRect.x + imgRect.w && py >= imgRect.y && py <= imgRect.y + imgRect.h) {
      // check resize handles
      if (Math.hypot(imgRect.x + imgRect.w - px, imgRect.y + imgRect.h - py) < 14) {
        return { type: 'image-resize' };
      }
      return { type: 'image' };
    }
    return { type: 'stage' };
  };

  const onMouseDown = (e) => {
    if (e.button !== 0) return;
    const hit = hitTest(e.clientX, e.clientY);
    const startX = e.clientX, startY = e.clientY;
    if (hit.type === 'image') {
      setDrag({ kind: 'image', startX, startY, startMmX: imageTransform.offsetMmX, startMmY: imageTransform.offsetMmY });
    } else if (hit.type === 'image-resize') {
      setDrag({ kind: 'image-resize', startX, startY, startW: imageTransform.widthMm, startH: imageTransform.heightMm, aspect: imageTransform.widthMm / imageTransform.heightMm });
    } else if (hit.type === 'monitor') {
      setSelectedMonitorId(hit.monitorId);
      const m = monitors.find(x => x.id === hit.monitorId);
      setDrag({ kind: 'monitor', startX, startY, startMmX: m.xMm, startMmY: m.yMm, monitorId: m.id });
    } else if (hit.type === 'handle' && hit.handle === 'rotate') {
      // Rotate 90 CW
      rotateMonitor(hit.monitorId, 90);
    } else if (hit.type === 'stage') {
      setSelectedMonitorId(null);
      setDrag({ kind: 'pan', startX, startY, startOx: panOffset.x, startOy: panOffset.y });
    }
  };

  const rotateMonitor = (id, delta) => {
    setMonitors(ms => ms.map(m => {
      if (m.id !== id) return m;
      return { ...m, rotation: (m.rotation + delta + 360) % 360 };
    }));
  };

  const onMouseMove = (e) => {
    const rect = stageRef.current.getBoundingClientRect();
    const px = e.clientX - rect.left;
    const py = e.clientY - rect.top;

    // Cursor / hover
    if (!drag) {
      const hit = hitTest(e.clientX, e.clientY);
      if (hit.type === 'monitor') {
        setHoveredMonitorId(hit.monitorId);
        const m = monitors.find(x => x.id === hit.monitorId);
        setTipPos({ x: px + 14, y: py + 14, monitor: m });
        stageRef.current.style.cursor = 'grab';
      } else if (hit.type === 'image') {
        setHoveredMonitorId(null); setTipPos(null);
        stageRef.current.style.cursor = 'move';
      } else if (hit.type === 'image-resize') {
        stageRef.current.style.cursor = 'nwse-resize';
        setHoveredMonitorId(null); setTipPos(null);
      } else if (hit.type === 'handle') {
        stageRef.current.style.cursor = 'crosshair';
      } else {
        setHoveredMonitorId(null); setTipPos(null);
        stageRef.current.style.cursor = 'default';
      }
      return;
    }

    const dx = e.clientX - drag.startX;
    const dy = e.clientY - drag.startY;
    const dxMm = dx / layout.scale;
    const dyMm = dy / layout.scale;

    if (drag.kind === 'image') {
      setImageTransform(t => ({ ...t, offsetMmX: drag.startMmX + dxMm, offsetMmY: drag.startMmY + dyMm }));
    } else if (drag.kind === 'image-resize') {
      const newW = Math.max(200, drag.startW + dxMm);
      const newH = newW / drag.aspect;
      setImageTransform(t => ({ ...t, widthMm: newW, heightMm: newH }));
    } else if (drag.kind === 'monitor') {
      let newX = drag.startMmX + dxMm;
      let newY = drag.startMmY + dyMm;
      // Snap to neighbours
      const others = monitors.filter(m => m.id !== drag.monitorId);
      const me = monitors.find(m => m.id === drag.monitorId);
      const meR = monRect({ ...me, xMm: newX, yMm: newY });
      const snapDist = 8 / layout.scale; // 8px in mm
      const newGuides = [];
      for (const o of others) {
        const oR = monRect(o);
        // Top edges align
        if (Math.abs(meR.y - oR.y) < snapDist) { newY = oR.y; newGuides.push({ kind: 'h', y: oR.y }); }
        // Bottom edges align
        if (Math.abs(meR.y + meR.h - (oR.y + oR.h)) < snapDist) { newY = oR.y + oR.h - meR.h; newGuides.push({ kind: 'h', y: oR.y + oR.h }); }
        // Left edges
        if (Math.abs(meR.x - oR.x) < snapDist) { newX = oR.x; newGuides.push({ kind: 'v', x: oR.x }); }
        // Right edges
        if (Math.abs(meR.x + meR.w - (oR.x + oR.w)) < snapDist) { newX = oR.x + oR.w - meR.w; newGuides.push({ kind: 'v', x: oR.x + oR.w }); }
        // Snap to bezel gap distance horizontally
        const gapTargets = [bezelMm.h];
        for (const g of gapTargets) {
          // To right of o
          if (Math.abs(meR.x - (oR.x + oR.w + g)) < snapDist) { newX = oR.x + oR.w + g; }
          // To left of o
          if (Math.abs(meR.x + meR.w - (oR.x - g)) < snapDist) { newX = oR.x - g - meR.w; }
        }
      }
      if (e.altKey) { newX = drag.startMmX + dxMm; newY = drag.startMmY + dyMm; }
      setGuides(newGuides);
      setMonitors(ms => ms.map(m => m.id === drag.monitorId ? { ...m, xMm: newX, yMm: newY } : m));
    } else if (drag.kind === 'pan') {
      setPanOffset({ x: drag.startOx + dx, y: drag.startOy + dy });
    }
  };

  const onMouseUp = () => {
    setDrag(null);
    setGuides([]);
  };

  // Wheel zoom
  const onWheel = (e) => {
    e.preventDefault();
    const delta = -e.deltaY * 0.001;
    const next = Math.max(0.5, Math.min(2.0, zoom * (1 + delta)));
    setZoom(next);
  };

  // Render helpers
  const renderMonitor = (m) => {
    const r = monRect(m);
    const a = mm2px(r.x, r.y);
    const b = mm2px(r.x + r.w, r.y + r.h);
    const w = b.x - a.x, h = b.y - a.y;
    const isSelected = selectedMonitorId === m.id;
    const isHovered = hoveredMonitorId === m.id;
    const isFlashing = flashedAt && Date.now() - flashedAt < 500;

    return (
      <div key={m.id} style={{
        position: 'absolute',
        left: a.x, top: a.y, width: w, height: h,
        border: `1.5px solid ${isSelected ? 'var(--accent)' : isHovered ? 'var(--text-2)' : 'var(--line-2)'}`,
        borderRadius: 3,
        boxShadow: isSelected
          ? '0 0 0 1px color-mix(in oklab, var(--accent) 30%, transparent), 0 0 24px color-mix(in oklab, var(--accent) 25%, transparent)'
          : isHovered
          ? '0 0 12px oklch(1 0 0 / 0.15)'
          : 'none',
        transition: 'border-color 80ms, box-shadow 80ms',
        pointerEvents: 'none',
        animation: isFlashing && applyFlashKey ? 'applyFlash 380ms ease-out' : 'none',
      }}>
        {/* Bezel ring */}
        <div style={{
          position: 'absolute', inset: 4,
          border: '1px solid oklch(0 0 0 / 0.4)',
          borderRadius: 1,
          pointerEvents: 'none',
        }} />
        {/* Label */}
        <div style={{
          position: 'absolute', top: 6, left: 8,
          fontSize: 10, fontWeight: 600, letterSpacing: '0.04em',
          color: isSelected ? 'var(--accent)' : 'var(--text-2)',
          textShadow: '0 1px 2px oklch(0 0 0 / 0.6)',
          fontFamily: 'var(--mono)',
          pointerEvents: 'none',
        }}>
          {m.name}{m.primary ? ' ★' : ''}
        </div>
        {/* Resolution badge bottom-right */}
        <div style={{
          position: 'absolute', bottom: 6, right: 8,
          fontSize: 9, color: 'var(--text-3)',
          fontFamily: 'var(--mono)',
          textShadow: '0 1px 2px oklch(0 0 0 / 0.6)',
        }}>
          {(m.rotation === 90 || m.rotation === 270) ? `${m.hPx}×${m.wPx}` : `${m.wPx}×${m.hPx}`}
        </div>
        {/* Rotate handle when selected */}
        {isSelected && (
          <div style={{
            position: 'absolute', top: -22, right: -4,
            width: 18, height: 18, borderRadius: '50%',
            background: 'var(--accent)',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            color: 'oklch(0.16 0.01 250)', fontSize: 11, fontWeight: 700,
            boxShadow: '0 2px 6px oklch(0 0 0 / 0.4)',
            pointerEvents: 'auto', cursor: 'pointer',
          }} onClick={(e) => { e.stopPropagation(); rotateMonitor(m.id, 90); }} title="Rotate 90°">↻</div>
        )}
      </div>
    );
  };

  // Dimension lines between monitors (CAD style)
  const dimLines = useMemo(() => {
    if (!showDimsAlways && !drag && !hoveredMonitorId && !selectedMonitorId) return [];
    const lines = [];
    // For each adjacent pair (sorted by x), show horizontal gap
    const sorted = [...monitors].sort((a, b) => a.xMm - b.xMm);
    for (let i = 0; i < sorted.length - 1; i++) {
      const a = sorted[i], b = sorted[i + 1];
      const ar = monRect(a), br = monRect(b);
      const gap = br.x - (ar.x + ar.w);
      if (gap > 0) {
        const yMm = Math.max(ar.y, br.y) + Math.min(ar.h, br.h) / 2;
        lines.push({
          x1Mm: ar.x + ar.w, y1Mm: yMm,
          x2Mm: br.x, y2Mm: yMm,
          label: fmtMm(gap),
        });
      }
    }
    return lines;
  }, [monitors, showDimsAlways, drag, hoveredMonitorId, selectedMonitorId]);

  return (
    <div
      ref={stageRef}
      onMouseDown={onMouseDown}
      onMouseMove={onMouseMove}
      onMouseUp={onMouseUp}
      onMouseLeave={() => { onMouseUp(); setHoveredMonitorId(null); setTipPos(null); }}
      onWheel={onWheel}
      style={{
        position: 'absolute', inset: 0,
        background: `
          radial-gradient(ellipse at 50% 40%, color-mix(in oklab, var(--bg-2) 100%, transparent), var(--bg) 70%),
          var(--bg)
        `,
        overflow: 'hidden',
        userSelect: 'none',
        cursor: 'default',
      }}
    >
      {/* Grid */}
      <CanvasGrid scale={layout.scale} ox={layout.ox} oy={layout.oy} stage={stageSize} />

      {/* Image (panorama) */}
      <div style={{
        position: 'absolute',
        left: imgRect.x, top: imgRect.y, width: imgRect.w, height: imgRect.h,
        backgroundImage: `url(${panoUrl})`,
        backgroundSize: 'cover',
        backgroundPosition: 'center',
        opacity: dimOff ? 0.18 : 1,
        transition: drag ? 'none' : 'opacity 200ms ease',
        boxShadow: dimOff ? 'none' : '0 0 0 1px oklch(1 0 0 / 0.06)',
        pointerEvents: 'none',
      }} />

      {/* "What you'll see" — image visible only inside monitor cutouts when dimmed */}
      {dimOff && monitors.map(m => {
        const r = monRect(m);
        const a = mm2px(r.x, r.y);
        const b = mm2px(r.x + r.w, r.y + r.h);
        return (
          <div key={'cut-' + m.id} style={{
            position: 'absolute',
            left: a.x, top: a.y, width: b.x - a.x, height: b.y - a.y,
            backgroundImage: `url(${panoUrl})`,
            backgroundSize: `${imgRect.w}px ${imgRect.h}px`,
            backgroundPosition: `${imgRect.x - a.x}px ${imgRect.y - a.y}px`,
            backgroundRepeat: 'no-repeat',
            pointerEvents: 'none',
          }} />
        );
      })}

      {/* Image resize handle */}
      <div style={{
        position: 'absolute',
        left: imgRect.x + imgRect.w - 6, top: imgRect.y + imgRect.h - 6,
        width: 12, height: 12, borderRadius: 3,
        background: 'var(--accent)',
        border: '2px solid var(--bg)',
        cursor: 'nwse-resize',
        pointerEvents: 'none',
        opacity: 0.85,
      }} />

      {/* Monitors */}
      {monitors.map(renderMonitor)}

      {/* Dimension lines (CAD style) */}
      <DimensionLines lines={dimLines} mm2px={mm2px} />

      {/* Alignment guides while dragging monitor */}
      {guides.map((g, i) => {
        if (g.kind === 'h') {
          const p = mm2px(0, g.y);
          return <div key={i} style={{
            position: 'absolute', left: 0, right: 0, top: p.y,
            height: 1, background: 'var(--accent)',
            pointerEvents: 'none', opacity: 0.7,
          }} />;
        }
        const p = mm2px(g.x, 0);
        return <div key={i} style={{
          position: 'absolute', top: 0, bottom: 0, left: p.x,
          width: 1, background: 'var(--accent)',
          pointerEvents: 'none', opacity: 0.7,
        }} />;
      })}

      {/* Hover tooltip */}
      {tipPos && tipPos.monitor && (
        <div className="tip" style={{ left: tipPos.x, top: tipPos.y }}>
          <div style={{ fontWeight: 600, fontSize: 12 }}>{tipPos.monitor.name} — {tipPos.monitor.model}</div>
          <div className="mono" style={{ color: 'var(--text-2)', marginTop: 4 }}>
            {tipPos.monitor.wPx}×{tipPos.monitor.hPx} @ {tipPos.monitor.hz}Hz
          </div>
          <div className="mono" style={{ color: 'var(--text-3)', marginTop: 2 }}>
            {Math.round(tipPos.monitor.wMm)}×{Math.round(tipPos.monitor.hMm)} mm
          </div>
        </div>
      )}
    </div>
  );
}

function CanvasGrid({ scale, ox, oy, stage }) {
  // 100mm grid
  const step = 100 * scale;
  if (step < 8) return null;
  const startX = ox % step;
  const startY = oy % step;
  return (
    <svg style={{ position: 'absolute', inset: 0, pointerEvents: 'none', opacity: 0.35 }}>
      <defs>
        <pattern id="grid" x={startX} y={startY} width={step} height={step} patternUnits="userSpaceOnUse">
          <path d={`M ${step} 0 L 0 0 0 ${step}`} fill="none" stroke="var(--line)" strokeWidth="0.5" />
        </pattern>
      </defs>
      <rect width="100%" height="100%" fill="url(#grid)" />
    </svg>
  );
}

function DimensionLines({ lines, mm2px }) {
  return (
    <svg style={{ position: 'absolute', inset: 0, pointerEvents: 'none', overflow: 'visible' }}>
      {lines.map((l, i) => {
        const a = mm2px(l.x1Mm, l.y1Mm);
        const b = mm2px(l.x2Mm, l.y2Mm);
        const cx = (a.x + b.x) / 2, cy = (a.y + b.y) / 2;
        return (
          <g key={i} stroke="var(--accent)" strokeOpacity="0.85" fill="var(--accent)">
            {/* arrow line */}
            <line x1={a.x} y1={a.y} x2={b.x} y2={b.y} strokeWidth="1" strokeDasharray="3 3" />
            {/* end ticks */}
            <line x1={a.x} y1={a.y - 5} x2={a.x} y2={a.y + 5} strokeWidth="1" />
            <line x1={b.x} y1={b.y - 5} x2={b.x} y2={b.y + 5} strokeWidth="1" />
            {/* label background */}
            <rect x={cx - 24} y={cy - 9} width="48" height="18" rx="3"
                  fill="var(--bg)" stroke="var(--accent)" strokeOpacity="0.4" strokeWidth="0.5" />
            <text x={cx} y={cy + 4} textAnchor="middle"
                  fontFamily="var(--mono)" fontSize="10"
                  fill="var(--accent)" stroke="none"
                  fontWeight="600">
              {l.label}
            </text>
          </g>
        );
      })}
    </svg>
  );
}

window.MonitorPreviewCanvas = MonitorPreviewCanvas;
window.SP_DEFAULTS = { DEFAULT_MONITORS, PANO_W, PANO_H, MM27 };
window.SP_buildPanoDataUrl = buildPanoDataUrl;
window.SP_monRect = monRect;
window.SP_bbox = bbox;
window.SP_fmtMm = fmtMm;
