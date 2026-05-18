/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{svelte,js,ts}"],
  theme: {
    extend: {
      colors: {
        // 背景层级 (CSS 变量驱动主题切换)
        "nx-deep": "var(--nx-deep)",
        "nx-bg": "var(--nx-bg)",
        "nx-surface": "var(--nx-surface)",
        "nx-raised": "var(--nx-raised)",
        "nx-overlay": "var(--nx-overlay)",
        // 边框
        "nx-border": "var(--nx-border)",
        "nx-border-light": "var(--nx-border-light)",
        // 文字
        "nx-text": "var(--nx-text)",
        "nx-text-secondary": "var(--nx-text-secondary)",
        "nx-text-muted": "var(--nx-text-muted)",
        // 语义色
        "nx-accent": "var(--nx-accent)",
        "nx-success": "var(--nx-success)",
        "nx-danger": "var(--nx-danger)",
        "nx-warning": "var(--nx-warning)",
        "nx-info": "var(--nx-info)",
      },
      fontFamily: {
        sans: ['"Inter"', 'system-ui', '-apple-system', 'sans-serif'],
        mono: ['"JetBrains Mono"', '"Menlo"', 'monospace'],
      },
      fontSize: {
        xs: ["0.75rem", { lineHeight: "1rem" }],
        sm: ["0.875rem", { lineHeight: "1.25rem" }],
        base: ["0.875rem", { lineHeight: "1.5rem" }],
        lg: ["1rem", { lineHeight: "1.5rem" }],
        xl: ["1.125rem", { lineHeight: "1.5rem" }],
        "2xl": ["1.25rem", { lineHeight: "1.5rem" }],
      },
      borderRadius: {
        DEFAULT: "0",
        nx: "0",
        none: "0",
        sm: "0",
        md: "0",
        lg: "0",
        xl: "0",
        "2xl": "0",
        "3xl": "0",
        full: "0",
      },
      spacing: {
        "1px": "1px",
      },
    },
  },
  plugins: [],
};
