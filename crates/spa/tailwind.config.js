/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./index.html",
    "./src/**/*.rs",
  ],
  theme: {
    extend: {
      colors: {
        cream: { DEFAULT: '#F5F0E8', light: '#FAF8F4' },
        ink: { DEFAULT: '#1A1915' },
        brick: { DEFAULT: '#C0392B', hover: '#A93226' },
        teal: { DEFAULT: '#1B6B5A', light: '#E8F5F0' },
        ochre: { DEFAULT: '#D4940A', light: '#FFF8E7' },
        'warm-gray': { DEFAULT: '#D5CFC3', light: '#E8E4DB' },
      },
      fontFamily: {
        display: ['Bitter', 'Georgia', 'serif'],
        body: ['DM Sans', '-apple-system', 'sans-serif'],
      },
      borderWidth: {
        'grumps': '2px',
      },
    },
  },
  plugins: [],
}
