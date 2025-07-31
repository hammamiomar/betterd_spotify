/** @type {import('tailwindcss').Config} */
module.exports = {
    content: [
        "./src/**/*.{rs,html,css}",
        "./dist/**/*.html",
    ],
    darkMode: 'media',
    theme: {
        extend: {
            colors: {
                'spotify-green': '#1DB954',
                'sage': {
                    50: '#f7f9f5',
                    100: '#eef3ea',
                    200: '#dde7d5',
                    300: '#c1d4b6',
                    400: '#9fc08e',
                    500: '#7fa86d',
                    600: '#648a54',
                    700: '#4f6d44',
                    800: '#3d5435',
                    900: '#2c3e28',
                    950: '#1a2218',
                }
            },
            fontFamily: {
                'sans': ['Inter', 'system-ui', 'sans-serif'],
            },
            animation: {
                'float': 'float 6s ease-in-out infinite',
                'pulse-sage': 'pulse-sage 2s cubic-bezier(0.4, 0, 0.6, 1) infinite',
                'sparkle': 'sparkle 2s ease-in-out infinite',
            },
            keyframes: {
                float: {
                    '0%, 100%': { transform: 'translateY(0px)' },
                    '50%': { transform: 'translateY(-10px)' },
                },
                'pulse-sage': {
                    '0%, 100%': { 
                        opacity: '1',
                        backgroundColor: 'rgb(193, 212, 182)',
                    },
                    '50%': { 
                        opacity: '0.7',
                        backgroundColor: 'rgb(159, 192, 142)',
                    },
                },
                sparkle: {
                    '0%, 100%': { 
                        transform: 'scale(1) rotate(0deg)',
                        opacity: '0.8',
                    },
                    '25%': { 
                        transform: 'scale(1.1) rotate(90deg)',
                        opacity: '1',
                    },
                    '50%': { 
                        transform: 'scale(1.2) rotate(180deg)',
                        opacity: '0.9',
                    },
                    '75%': { 
                        transform: 'scale(1.1) rotate(270deg)',
                        opacity: '1',
                    },
                },
            },
            backdropBlur: {
                'xs': '2px',
            },
            animationDelay: {
                '75': '75ms',
                '150': '150ms',
            }
        },
    },
    plugins: [],
}