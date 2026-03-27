/** @type {import('tailwindcss').Config} */
export default {
  content: ['./src/**/*.{html,js,svelte,ts}'],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        // 深色模式 - 深灰
        dark: {
          bg: '#1a1a1a',
          surface: '#242424',
          border: '#333333',
          muted: '#666666',
        },
        // 浅色模式 - 米白
        light: {
          bg: '#faf8f5',
          surface: '#ffffff',
          border: '#e5e2dd',
          muted: '#9a9893',
        },
        // 品牌色
        brand: {
          primary: '#ff6b35',
          secondary: '#4a90d9',
          accent: '#2dd4bf',
        },
      },
      fontFamily: {
        sans: ['Inter', 'system-ui', '-apple-system', 'sans-serif'],
        mono: ['JetBrains Mono', 'Consolas', 'monospace'],
      },
      backdropBlur: {
        glass: '20px',
      },
    },
  },
  plugins: [],
};
