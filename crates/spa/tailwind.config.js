/** @type {import('tailwindcss').Config} */
module.exports = {
  darkMode: ['selector', '[data-theme="dark"]'],
  content: [
    "./index.html",
    "./src/**/*.rs",
  ],
  theme: {
    extend: {
      colors: {
        // Primitives (kept for explicit uses)
        cream: { DEFAULT: '#F5F0E8', light: '#FAF8F4' },
        ink: { DEFAULT: '#1A1915' },
        brick: { DEFAULT: '#C0392B', hover: '#A93226' },
        teal: { DEFAULT: '#1B6B5A', light: '#E8F5F0' },
        ochre: { DEFAULT: '#D4940A', light: '#FFF8E7' },
        'warm-gray': { DEFAULT: '#D5CFC3', light: '#E8E4DB' },

        // Semantic tokens — flat keys so Tailwind generates clean
        // utility names (bg-surface, text-primary, border-strong, …).
        // CSS variables resolve the correct value per theme.
        surface: {
          DEFAULT: 'var(--surface-base)',
          raised: 'var(--surface-raised)',
        },
        primary:   'var(--text-primary)',
        secondary: 'var(--text-secondary)',
        muted:     'var(--text-muted)',
        strong:    'var(--border-strong)',
        subtle:    'var(--border-subtle)',
        'hover-tint': 'var(--hover-tint)',
        accent: {
          DEFAULT: 'var(--accent-primary)',
          hover:   'var(--accent-primary-hover)',
        },
        success: {
          DEFAULT: 'var(--accent-success)',
          bg:      'var(--accent-success-bg)',
        },
        warning: {
          DEFAULT: 'var(--accent-warning)',
          bg:      'var(--accent-warning-bg)',
        },
      },
      fontFamily: {
        display: ['Bitter', 'Georgia', 'serif'],
        body: ['DM Sans', '-apple-system', 'sans-serif'],
      },
      borderWidth: {
        'grumps': '2px',
      },
      borderRadius: {
        sm: '3px',
      },
    },
  },
  plugins: [],
}
