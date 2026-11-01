/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      fontFamily: {
        'sans': ['Segoe UI', 'ui-sans-serif', 'system-ui', '-apple-system', 'BlinkMacSystemFont', 'Inter', 'sans-serif'],
        'mono': ['Consolas', 'Cascadia Code', 'ui-monospace', 'monospace'],
      },
      colors: {
        background: "#0a0d14",
        surface: "#111726",
        "surface-light": "#192238",
        border: "#202d4a",
        primary: {
          DEFAULT: "#10b981", // Emerald 500
          hover: "#059669",
          glow: "rgba(16, 185, 129, 0.2)",
        },
        accent: {
          cyan: "#06b6d4",
          amber: "#f59e0b",
          rose: "#f43f5e",
        }
      },
      animation: {
        'shimmer': 'shimmer 1.5s linear infinite',
      },
      keyframes: {
        shimmer: {
          '0%': { transform: 'translateX(-100%)' },
          '100%': { transform: 'translateX(300%)' },
        },
      },
    },
  },
  plugins: [],
}

