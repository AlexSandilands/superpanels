// Persistent UI preferences: theme, accent, density, dim-always, blur.
// Mirrored to <html data-theme=… data-density=… data-blur=…> + a CSS custom
// property for accent so the design tokens in `app.css` pick them up.

export type Theme = 'auto' | 'light' | 'dark';
export type Density = 'compact' | 'regular' | 'spacious';

const KEY = 'superpanels.ui.v1';
const DEFAULT_ACCENT = '#3daee9';

export const ACCENT_OPTIONS = [
  '#3daee9',
  '#7c5cff',
  '#34d399',
  '#ff7849',
  '#e8e8e8',
  '#f0b6c5',
] as const;

type Persisted = {
  theme: Theme;
  accent: string;
  density: Density;
  dimsAlways: boolean;
  followSystemAccent: boolean;
  windowBlur: boolean;
  trayRun: boolean;
  notify: 'off' | 'errors only' | 'all';
  motion: 'system' | 'on' | 'off';
  locale: string;
};

function load(): Persisted {
  const fallback: Persisted = {
    theme: 'dark',
    accent: DEFAULT_ACCENT,
    density: 'regular',
    dimsAlways: true,
    followSystemAccent: false,
    windowBlur: true,
    trayRun: true,
    notify: 'errors only',
    motion: 'system',
    locale: 'en-US (system)',
  };
  if (typeof window === 'undefined') return fallback;
  try {
    const raw = window.localStorage?.getItem(KEY);
    if (!raw) return fallback;
    const parsed = JSON.parse(raw) as Partial<Persisted>;
    return { ...fallback, ...parsed };
  } catch {
    return fallback;
  }
}

let state = $state<Persisted>(load());

function persist() {
  try {
    window.localStorage?.setItem(KEY, JSON.stringify(state));
  } catch {
    // localStorage unavailable; preferences live for the session only.
  }
}

function applyTheme(theme: Theme): 'light' | 'dark' {
  if (theme !== 'auto') return theme;
  return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}

export function applyDocumentTokens() {
  if (typeof document === 'undefined') return;
  document.documentElement.dataset.theme = applyTheme(state.theme);
  document.documentElement.dataset.density = state.density;
  document.documentElement.dataset.blur = state.windowBlur ? 'on' : 'off';
  document.documentElement.style.setProperty('--accent', state.accent);
}

export const ui = {
  get theme() {
    return state.theme;
  },
  get accent() {
    return state.accent;
  },
  get density() {
    return state.density;
  },
  get dimsAlways() {
    return state.dimsAlways;
  },
  get followSystemAccent() {
    return state.followSystemAccent;
  },
  get windowBlur() {
    return state.windowBlur;
  },
  get trayRun() {
    return state.trayRun;
  },
  get notify() {
    return state.notify;
  },
  get motion() {
    return state.motion;
  },
  get locale() {
    return state.locale;
  },

  set(patch: Partial<Persisted>) {
    state = { ...state, ...patch };
    persist();
    applyDocumentTokens();
  },
};
