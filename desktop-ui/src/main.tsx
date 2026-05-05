import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { Toaster } from 'sonner'
import App from './App'
import './styles/globals.css'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
    <Toaster 
      position="top-right" 
      toastOptions={{
        style: {
          background: '#151A25',
          border: '1px solid #2A3142',
          color: '#F1F5F9',
          borderRadius: '12px',
          boxShadow: '0 8px 32px rgba(0,0,0,0.4)',
          fontFamily: 'Inter, sans-serif',
        },
      }}
    />
  </StrictMode>,
)
