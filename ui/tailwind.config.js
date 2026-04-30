/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{svelte,ts}'],
  darkMode: 'media',
  theme: {
    extend: {
      colors: {
        bezel: '#0b1220',
        canvas: '#0f172a',
        accent: '#60a5fa',
      },
    },
  },
  plugins: [],
};
