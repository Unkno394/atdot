'use client'
import Link from 'next/link'
import { usePathname, useRouter } from 'next/navigation'
import { logout, clearSession } from '@/lib/api'

const NAV = [
  { href: '/dashboard', label: 'Обзор',      icon: '◎' },
  { href: '/keys',      label: 'Интеграция', icon: '⬡' },
  { href: '/profile',   label: 'Профиль',    icon: '◈' },
]

export default function Sidebar() {
  const pathname = usePathname()
  const router   = useRouter()

  async function handleLogout() {
    try { await logout() } catch { /* ignore */ }
    clearSession()
    router.push('/auth/login')
  }

  return (
    <aside style={{
      width: 220, minHeight: '100vh', flexShrink: 0,
      background: 'rgba(0,0,0,0.5)',
      backdropFilter: 'blur(24px)',
      borderRight: '1px solid rgba(255,255,255,0.07)',
      display: 'flex', flexDirection: 'column',
      padding: '28px 16px',
      position: 'sticky', top: 0, height: '100vh',
    }}>
      <div style={{
        fontWeight: 700, fontSize: 18, marginBottom: 44,
        paddingLeft: 12, letterSpacing: '-0.02em', color: '#fff',
      }}>
        <span style={{ color: '#a78bfa' }}>AT</span>dot
      </div>

      <nav style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 4 }}>
        {NAV.map(({ href, label, icon }) => {
          const active = pathname === href
          return (
            <Link key={href} href={href} style={{
              display: 'flex', alignItems: 'center', gap: 10,
              padding: '10px 12px', borderRadius: 10, textDecoration: 'none',
              fontSize: 14, fontWeight: active ? 600 : 400,
              color: active ? '#fff' : 'rgba(255,255,255,0.45)',
              background: active ? 'rgba(124,58,237,0.25)' : 'transparent',
              border: active ? '1px solid rgba(124,58,237,0.35)' : '1px solid transparent',
              transition: 'all 0.15s ease',
            }}>
              <span style={{ color: active ? '#a78bfa' : 'rgba(255,255,255,0.3)', fontSize: 16 }}>
                {icon}
              </span>
              {label}
            </Link>
          )
        })}
      </nav>

      <button onClick={handleLogout} style={{
        background: 'none', border: '1px solid rgba(255,255,255,0.07)',
        cursor: 'pointer', color: 'rgba(255,255,255,0.35)',
        fontSize: 13, textAlign: 'left',
        padding: '10px 12px', borderRadius: 10,
        display: 'flex', alignItems: 'center', gap: 10,
        transition: 'all 0.15s ease',
      }}>
        <span>↩</span> Выйти
      </button>
    </aside>
  )
}
