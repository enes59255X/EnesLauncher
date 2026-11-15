/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        sans: [
          "Segoe UI Variable Display",
          "Segoe UI",
          "ui-sans-serif",
          "system-ui",
          "-apple-system",
          "BlinkMacSystemFont",
          "Inter",
          "sans-serif",
        ],
        mono: ["Cascadia Mono", "Consolas", "ui-monospace", "monospace"],
      },
      colors: {
        // Nötr yüzeyler (index.css tokenlarıyla aynı değerler)
        canvas: "#0a0b0d",
        background: "#0a0b0d",
        surface: {
          DEFAULT: "#101114",
          2: "#16181c",
          3: "#1d2025",
        },
        ink: {
          DEFAULT: "#e9ebef",
          muted: "#99a1ad",
          dim: "#69707c",
        },
        // İnce ayırıcı çizgiler
        line: {
          DEFAULT: "#23262c",
          strong: "#33373f",
        },
        // Tema vurgusu: CSS değişkeninden gelir
        accent: {
          DEFAULT: "var(--accent)",
          hover: "var(--accent-hover)",
          soft: "var(--accent-soft)",
        },
      },
      borderRadius: {
        lg: "10px",
        xl: "12px",
        "2xl": "16px",
        "3xl": "20px",
      },
      transitionDuration: {
        DEFAULT: "150ms",
      },
    },
  },
  plugins: [],
};
