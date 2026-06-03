'use client'
import LetterGlitch from './LetterGlitch'
import Sidebar from './Sidebar'

export default function AppShell({ children }: { children: React.ReactNode }) {
  return (
    <div style={{ minHeight: '100vh', position: 'relative', color: '#fff' }}>
      {/* fixed glitch background */}
      <div style={{ position: 'fixed', inset: 0, zIndex: 0 }}>
        <LetterGlitch
          glitchColors={['#2b4539', '#61dca3', '#61b3dc']}
          glitchSpeed={150}
          centerVignette={false}
          outerVignette={true}
          smooth={true}
          characters="ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
        />
        <div style={{ position: 'absolute', inset: 0, background: 'rgba(0,0,0,0.82)' }} />
      </div>

      {/* layout */}
      <div style={{ position: 'relative', zIndex: 1, display: 'flex', minHeight: '100vh' }}>
        <Sidebar />
        <main style={{ flex: 1, padding: '40px 48px', overflowY: 'auto', maxHeight: '100vh' }}>
          {children}
        </main>
      </div>
    </div>
  )
}
