/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        // Тёмная тема — профессиональные цвета
        bg: {
          primary: '#0a0e17',    // Основной фон
          secondary: '#111827',  // Карточки
          tertiary: '#1f2937',   // Ховер
          accent: '#1e3a5f',     // Акцент
        },
        brand: {
          primary: '#3b82f6',    // Синий
          secondary: '#8b5cf6',  // Фиолетовый
          success: '#10b981',    // Зелёный
          danger: '#ef4444',     // Красный
          warning: '#f59e0b',    // Жёлтый
          info: '#06b6d4',       // Голубой
        },
        text: {
          primary: '#f9fafb',
          secondary: '#9ca3af',
          muted: '#6b7280',
        }
      },
      fontFamily: {
        mono: ['JetBrains Mono', 'Consolas', 'monospace'],
        sans: ['Inter', 'system-ui', 'sans-serif'],
      },
      animation: {
        'pulse-slow': 'pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite',
        'slide-in': 'slideIn 0.3s ease-out',
        'fade-in': 'fadeIn 0.2s ease-in',
      },
      keyframes: {
        slideIn: {
          '0%': { transform: 'translateY(-10px)', opacity: '0' },
          '100%': { transform: 'translateY(0)', opacity: '1' },
        },
        fadeIn: {
          '0%': { opacity: '0' },
          '100%': { opacity: '1' },
        },
      },
    },
  },
  plugins: [],
}
