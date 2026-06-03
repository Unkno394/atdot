'use client'
import { useEffect, useState } from 'react'
import { useRouter } from 'next/navigation'
import AppShell from '@/components/AppShell'
import Modal from '@/components/Modal'
import { getMe, logout, clearSession, updateEmail, changePassword, type Me } from '@/lib/api'

const glass = {
  background: 'rgba(0,0,0,0.35)',
  backdropFilter: 'blur(16px)',
  border: '1px solid rgba(255,255,255,0.08)',
  borderRadius: 16,
} as const

function Avatar({ email }: { email: string }) {
  const letter = email ? email[0].toUpperCase() : '?'
  return (
    <div style={{
      width: 72, height: 72, borderRadius: '50%',
      background: 'rgba(124,58,237,0.3)',
      border: '2px solid rgba(124,58,237,0.5)',
      display: 'flex', alignItems: 'center', justifyContent: 'center',
      fontSize: 28, fontWeight: 700, color: '#a78bfa',
      flexShrink: 0,
    }}>
      {letter}
    </div>
  )
}

export default function ProfilePage() {
  const router = useRouter()
  const [me,          setMe]          = useState<Me | null>(null)
  const [error,       setError]       = useState('')

  const [emailModalOpen,    setEmailModalOpen]    = useState(false)
  const [passwordModalOpen, setPasswordModalOpen] = useState(false)

  const [newEmail,    setNewEmail]    = useState('')
  const [emailSaving, setEmailSaving] = useState(false)
  const [emailMsg,    setEmailMsg]    = useState('')

  const [currentPassword,  setCurrentPassword]  = useState('')
  const [newPassword,      setNewPassword]      = useState('')
  const [confirmPassword,  setConfirmPassword]  = useState('')
  const [passwordSaving,   setPasswordSaving]   = useState(false)
  const [passwordMsg,      setPasswordMsg]      = useState('')

  const [showCurrent, setShowCurrent] = useState(false)
  const [showNew,     setShowNew]     = useState(false)
  const [showConfirm, setShowConfirm] = useState(false)

  useEffect(() => {
    if (!localStorage.getItem('mdr_token')) { router.push('/auth/login'); return }
    getMe().then(setMe).catch(e => setError(e.message))
  }, [router])

  async function handleEmailChange(e: React.FormEvent) {
    e.preventDefault()
    if (!newEmail.trim()) return
    setEmailSaving(true); setEmailMsg('')
    try {
      await updateEmail(newEmail.trim())
      localStorage.setItem('mdr_email', newEmail.trim())
      setMe(prev => prev ? { ...prev, email: newEmail.trim() } : prev)
      setEmailMsg('Email обновлён')
      setNewEmail('')
    } catch (e: any) {
      setEmailMsg(e.message)
    } finally {
      setEmailSaving(false)
    }
  }

  async function handlePasswordChange(e: React.FormEvent) {
    e.preventDefault()
    if (newPassword !== confirmPassword) { setPasswordMsg('Пароли не совпадают'); return }
    if (newPassword.length < 8) { setPasswordMsg('Минимум 8 символов'); return }
    setPasswordSaving(true); setPasswordMsg('')
    try {
      await changePassword(currentPassword, newPassword)
      setPasswordMsg('Пароль обновлён')
      setCurrentPassword(''); setNewPassword(''); setConfirmPassword('')
    } catch (e: any) { setPasswordMsg(e.message) }
    finally { setPasswordSaving(false) }
  }

  async function handleLogout() {
    try { await logout() } catch { /* ignore */ }
    clearSession()
    router.push('/auth/login')
  }

  const email     = me?.email      ?? ''
  const sessionId = me?.session_id ?? localStorage.getItem('mdr_session_id') ?? '—'
  const userId    = me?.user_id    ?? '—'

  const inputStyle: React.CSSProperties = {
    width: '100%', background: 'rgba(255,255,255,0.06)',
    border: '1px solid rgba(255,255,255,0.12)',
    borderRadius: 10, padding: '11px 16px',
    color: '#fff', fontSize: 14, outline: 'none', boxSizing: 'border-box',
  }

  const primaryBtn: React.CSSProperties = {
    background: 'rgba(124,58,237,0.35)', border: '1px solid rgba(124,58,237,0.4)',
    color: '#fff', borderRadius: 10, padding: '12px 0',
    fontWeight: 600, fontSize: 14, cursor: 'pointer', width: '100%', transition: 'all 0.15s',
  }

  const eyeBtn: React.CSSProperties = {
    position: 'absolute', right: 12, top: '50%', transform: 'translateY(-50%)',
    background: 'none', border: 'none', cursor: 'pointer',
    color: 'rgba(255,255,255,0.4)', padding: 0, display: 'flex', alignItems: 'center',
  }

  function EyeIcon({ visible }: { visible: boolean }) {
    return visible ? (
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94"/>
        <path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19"/>
        <path d="M14.12 14.12a3 3 0 1 1-4.24-4.24"/>
        <line x1="1" y1="1" x2="23" y2="23"/>
      </svg>
    ) : (
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
        <circle cx="12" cy="12" r="3"/>
      </svg>
    )
  }

  return (
    <AppShell>
      <h1 style={{ fontSize: 26, fontWeight: 700, letterSpacing: '-0.03em', marginBottom: 6 }}>Профиль</h1>
      <p style={{ color: 'rgba(255,255,255,0.4)', fontSize: 14, marginBottom: 36 }}>
        Информация об аккаунте и текущей сессии
      </p>

      {error && (
        <div style={{ ...glass, padding: '12px 16px', color: '#f87171', marginBottom: 24, fontSize: 14, borderColor: 'rgba(248,113,113,0.2)' }}>
          {error}
        </div>
      )}

      <div style={{ ...glass, padding: '28px 32px', marginBottom: 16, display: 'flex', alignItems: 'center', gap: 24 }}>
        <Avatar email={email} />
        <div>
          <div style={{ fontSize: 20, fontWeight: 700, marginBottom: 4 }}>{email || '—'}</div>
          <div style={{ fontSize: 12, color: 'rgba(255,255,255,0.35)', fontFamily: 'monospace' }}>
            {userId}
          </div>
        </div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16, marginBottom: 16 }}>

        <div style={{ ...glass, padding: '28px 32px' }}>
          <div style={{ fontSize: 13, fontWeight: 700, color: 'rgba(255,255,255,0.45)', textTransform: 'uppercase', letterSpacing: '0.08em', marginBottom: 20 }}>
            Управление аккаунтом
          </div>
          <div style={{ display: 'flex', gap: 12 }}>
            <button
              onClick={() => setEmailModalOpen(true)}
              style={{
                flex: 1, background: 'rgba(124,58,237,0.2)', border: '1px solid rgba(124,58,237,0.3)',
                color: '#a78bfa', borderRadius: 10, padding: '11px 0',
                fontWeight: 600, fontSize: 14, cursor: 'pointer', transition: 'all 0.15s',
              }}
            >
              Сменить email
            </button>
            <button
              onClick={() => setPasswordModalOpen(true)}
              style={{
                flex: 1, background: 'rgba(124,58,237,0.2)', border: '1px solid rgba(124,58,237,0.3)',
                color: '#a78bfa', borderRadius: 10, padding: '11px 0',
                fontWeight: 600, fontSize: 14, cursor: 'pointer', transition: 'all 0.15s',
              }}
            >
              Сменить пароль
            </button>
          </div>
        </div>

        <div style={{ ...glass, padding: '28px 32px' }}>
          <div style={{ fontSize: 13, fontWeight: 700, color: 'rgba(255,255,255,0.45)', textTransform: 'uppercase', letterSpacing: '0.08em', marginBottom: 20 }}>
            Текущая сессия
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
            <div>
              <div style={{ fontSize: 11, color: 'rgba(255,255,255,0.3)', marginBottom: 6 }}>Session ID</div>
              <div style={{ fontSize: 12, fontFamily: 'monospace', color: '#a78bfa', wordBreak: 'break-all' }}>
                {sessionId}
              </div>
            </div>
            <div>
              <div style={{ fontSize: 11, color: 'rgba(255,255,255,0.3)', marginBottom: 6 }}>Истекает</div>
              <div style={{ fontSize: 13, color: 'rgba(255,255,255,0.7)' }}>через 7 дней</div>
            </div>
            <div style={{
              display: 'inline-flex', alignItems: 'center', gap: 6,
              background: 'rgba(97,220,163,0.1)', border: '1px solid rgba(97,220,163,0.2)',
              borderRadius: 8, padding: '6px 12px', width: 'fit-content',
            }}>
              <div style={{ width: 7, height: 7, borderRadius: '50%', background: '#61dca3' }} />
              <span style={{ fontSize: 12, color: '#61dca3', fontWeight: 600 }}>Активна</span>
            </div>
          </div>
        </div>
      </div>

      <div style={{ ...glass, padding: '28px 32px', borderColor: 'rgba(248,113,113,0.15)' }}>
        <div style={{ fontSize: 13, fontWeight: 700, color: 'rgba(248,113,113,0.7)', textTransform: 'uppercase', letterSpacing: '0.08em', marginBottom: 10 }}>
          Выход из аккаунта
        </div>
        <p style={{ color: 'rgba(255,255,255,0.35)', fontSize: 13, lineHeight: 1.6, marginBottom: 20 }}>
          Сессия будет аннулирована на сервере. Все активные токены перестанут работать.
        </p>
        <button
          onClick={handleLogout}
          style={{
            background: 'rgba(248,113,113,0.12)', border: '1px solid rgba(248,113,113,0.3)',
            color: '#f87171', borderRadius: 10, padding: '12px 28px',
            fontSize: 14, fontWeight: 600, cursor: 'pointer',
            backdropFilter: 'blur(8px)', transition: 'all 0.15s',
          }}
        >
          Выйти из аккаунта
        </button>
      </div>

      <Modal open={emailModalOpen} onClose={() => { setEmailModalOpen(false); setEmailMsg(''); setNewEmail('') }} title="Сменить email">
        <form onSubmit={handleEmailChange} style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          <div>
            <div style={{ fontSize: 12, color: 'rgba(255,255,255,0.4)', marginBottom: 8 }}>Текущий email</div>
            <div style={{ fontSize: 14, color: 'rgba(255,255,255,0.7)', padding: '11px 16px', background: 'rgba(255,255,255,0.04)', borderRadius: 10, border: '1px solid rgba(255,255,255,0.08)' }}>{email}</div>
          </div>
          <div>
            <div style={{ fontSize: 12, color: 'rgba(255,255,255,0.4)', marginBottom: 8 }}>Новый email</div>
            <input type="email" value={newEmail} onChange={e => setNewEmail(e.target.value)} placeholder="новый@email.com" style={inputStyle} required />
          </div>
          {emailMsg && <div style={{ fontSize: 13, color: emailMsg === 'Email обновлён' ? '#61dca3' : '#f87171' }}>{emailMsg}</div>}
          <button type="submit" disabled={emailSaving || !newEmail.trim()} style={primaryBtn}>
            {emailSaving ? 'Сохраняем...' : 'Сохранить'}
          </button>
        </form>
      </Modal>

      <Modal open={passwordModalOpen} onClose={() => { setPasswordModalOpen(false); setPasswordMsg(''); setCurrentPassword(''); setNewPassword(''); setConfirmPassword('') }} title="Сменить пароль">
        <form onSubmit={handlePasswordChange} style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          <div style={{ position: 'relative' }}>
            <input
              type={showCurrent ? 'text' : 'password'}
              value={currentPassword}
              onChange={e => setCurrentPassword(e.target.value)}
              placeholder="Текущий пароль"
              style={{ ...inputStyle, paddingRight: 40 }}
              required
            />
            <button type="button" onClick={() => setShowCurrent(v => !v)} style={eyeBtn} tabIndex={-1}>
              <EyeIcon visible={showCurrent} />
            </button>
          </div>
          <div style={{ position: 'relative' }}>
            <input
              type={showNew ? 'text' : 'password'}
              value={newPassword}
              onChange={e => setNewPassword(e.target.value)}
              placeholder="Новый пароль"
              style={{ ...inputStyle, paddingRight: 40 }}
              required
            />
            <button type="button" onClick={() => setShowNew(v => !v)} style={eyeBtn} tabIndex={-1}>
              <EyeIcon visible={showNew} />
            </button>
          </div>
          <div style={{ position: 'relative' }}>
            <input
              type={showConfirm ? 'text' : 'password'}
              value={confirmPassword}
              onChange={e => setConfirmPassword(e.target.value)}
              placeholder="Повторите новый пароль"
              style={{ ...inputStyle, paddingRight: 40 }}
              required
            />
            <button type="button" onClick={() => setShowConfirm(v => !v)} style={eyeBtn} tabIndex={-1}>
              <EyeIcon visible={showConfirm} />
            </button>
          </div>
          {passwordMsg && <div style={{ fontSize: 13, color: passwordMsg.includes('обновлён') ? '#61dca3' : '#f87171' }}>{passwordMsg}</div>}
          <button type="submit" disabled={passwordSaving} style={primaryBtn}>
            {passwordSaving ? 'Сохраняем...' : 'Сменить пароль'}
          </button>
        </form>
      </Modal>
    </AppShell>
  )
}
