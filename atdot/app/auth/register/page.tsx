'use client'

import { useState } from 'react'
import { useRouter } from 'next/navigation'
import LetterGlitch from '@/components/LetterGlitch'
import { login, register, saveSession } from '@/lib/api'

export default function AuthPage() {
  const router = useRouter()
  const [isRightPanelActive, setIsRightPanelActive] = useState(false)

  // login state
  const [loginEmail,         setLoginEmail]         = useState('')
  const [loginPassword,      setLoginPassword]      = useState('')
  const [loginError,         setLoginError]         = useState('')
  const [loginLoading,       setLoginLoading]       = useState(false)
  const [showLoginPassword,  setShowLoginPassword]  = useState(false)

  // register state
  const [regEmail,           setRegEmail]           = useState('')
  const [regPassword,        setRegPassword]        = useState('')
  const [regError,           setRegError]           = useState('')
  const [regLoading,         setRegLoading]         = useState(false)
  const [showRegPassword,    setShowRegPassword]    = useState(false)

  async function handleLogin(e: React.FormEvent) {
    e.preventDefault()
    setLoginLoading(true); setLoginError('')
    try {
      const result = await login(loginEmail, loginPassword)
      saveSession(result)
      router.push('/dashboard')
    } catch (err: any) {
      setLoginError(err.message)
    } finally {
      setLoginLoading(false)
    }
  }

  async function handleRegister(e: React.FormEvent) {
    e.preventDefault()
    if (regPassword.length < 8) {
      setRegError('Пароль должен быть не менее 8 символов'); return
    }
    setRegLoading(true); setRegError('')
    try {
      const result = await register(regEmail, regPassword)
      saveSession(result)
      router.push('/dashboard')
    } catch (err: any) {
      setRegError(err.message)
    } finally {
      setRegLoading(false)
    }
  }

  const input = "w-full max-w-sm bg-black/30 border border-white/10 rounded-xl py-4 px-5 my-2 text-white placeholder-white/30 focus:outline-none focus:border-emerald-400 transition"
  const btn   = "mt-6 px-10 py-4 rounded-full bg-emerald-500/20 hover:bg-emerald-500/30 border border-emerald-400/30 text-white font-semibold transition-all active:scale-95 backdrop-blur-sm disabled:opacity-50"

  return (
    <div className="relative w-full h-screen overflow-hidden bg-black">
      <div className={`relative w-full h-full overflow-hidden transition-all duration-700 ${isRightPanelActive ? 'right-panel-active' : ''}`}>

        {/* Register Form */}
        <div className={`absolute top-0 h-full transition-all duration-700 ease-in-out w-1/2 z-20 ${
          isRightPanelActive ? 'translate-x-full opacity-100' : 'left-0 opacity-0 pointer-events-none'
        }`}>
          <form
            onSubmit={handleRegister}
            className="flex items-center justify-center flex-col px-12 h-full text-center backdrop-blur-md bg-black/20"
          >
            <h1 className="font-bold text-4xl text-white mb-4">Регистрация</h1>

            <input
              type="email"
              placeholder="Email"
              value={regEmail}
              onChange={(e) => setRegEmail(e.target.value)}
              required
              className={input}
            />
            <div className="relative w-full max-w-sm">
              <input
                type={showRegPassword ? 'text' : 'password'}
                placeholder="Пароль (мин. 8 символов)"
                value={regPassword}
                onChange={(e) => setRegPassword(e.target.value)}
                required
                className="w-full max-w-sm bg-black/30 border border-white/10 rounded-xl py-4 px-5 my-2 text-white placeholder-white/30 focus:outline-none focus:border-emerald-400 transition pr-12"
              />
              <button
                type="button"
                onClick={() => setShowRegPassword(v => !v)}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-white/40 hover:text-white/70 transition"
                tabIndex={-1}
              >
                {showRegPassword ? (
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94"/>
                    <path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19"/>
                    <path d="M14.12 14.12a3 3 0 1 1-4.24-4.24"/>
                    <line x1="1" y1="1" x2="23" y2="23"/>
                  </svg>
                ) : (
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
                    <circle cx="12" cy="12" r="3"/>
                  </svg>
                )}
              </button>
            </div>

            {regError && (
              <p className="text-red-400 text-sm mt-2 max-w-sm">{regError}</p>
            )}

            <button type="submit" disabled={regLoading} className={btn}>
              {regLoading ? 'Загрузка...' : 'Зарегистрироваться'}
            </button>
          </form>
        </div>

        {/* Login Form */}
        <div className={`absolute top-0 h-full transition-all duration-700 ease-in-out w-1/2 z-20 ${
          isRightPanelActive ? 'translate-x-full opacity-0 pointer-events-none' : 'left-0 opacity-100'
        }`}>
          <form
            onSubmit={handleLogin}
            className="flex items-center justify-center flex-col px-12 h-full text-center backdrop-blur-md bg-black/20"
          >
            <h1 className="font-bold text-4xl text-white mb-4">Вход</h1>

            <input
              type="email"
              placeholder="Email"
              value={loginEmail}
              onChange={(e) => setLoginEmail(e.target.value)}
              required
              className={input}
            />
            <div className="relative w-full max-w-sm">
              <input
                type={showLoginPassword ? 'text' : 'password'}
                placeholder="Пароль"
                value={loginPassword}
                onChange={(e) => setLoginPassword(e.target.value)}
                required
                className="w-full max-w-sm bg-black/30 border border-white/10 rounded-xl py-4 px-5 my-2 text-white placeholder-white/30 focus:outline-none focus:border-emerald-400 transition pr-12"
              />
              <button
                type="button"
                onClick={() => setShowLoginPassword(v => !v)}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-white/40 hover:text-white/70 transition"
                tabIndex={-1}
              >
                {showLoginPassword ? (
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94"/>
                    <path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19"/>
                    <path d="M14.12 14.12a3 3 0 1 1-4.24-4.24"/>
                    <line x1="1" y1="1" x2="23" y2="23"/>
                  </svg>
                ) : (
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
                    <circle cx="12" cy="12" r="3"/>
                  </svg>
                )}
              </button>
            </div>

            {loginError && (
              <p className="text-red-400 text-sm mt-2 max-w-sm">{loginError}</p>
            )}

            <button type="submit" disabled={loginLoading} className={btn}>
              {loginLoading ? 'Загрузка...' : 'Войти'}
            </button>
          </form>
        </div>

        {/* Overlay */}
        <div className={`absolute top-0 left-1/2 w-1/2 h-full overflow-hidden transition-transform duration-700 ease-in-out z-30 ${
          isRightPanelActive ? '-translate-x-full' : ''
        }`}>
          <div
            className="relative -left-full h-full w-[200%] transition-transform duration-700 ease-in-out text-white"
            style={{ transform: isRightPanelActive ? 'translateX(50%)' : 'translateX(0)' }}
          >
            <div className="absolute inset-0">
              <LetterGlitch
                glitchColors={['#2b4539', '#61dca3', '#61b3dc']}
                glitchSpeed={120}
                centerVignette={false}
                outerVignette={true}
                smooth={true}
                characters="ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
              />
            </div>
            <div className="absolute inset-0 bg-black/50" />

            {/* Left panel — shown when register is active */}
            <div className={`absolute flex items-center justify-center flex-col px-10 text-center top-0 h-full w-1/2 transition-transform duration-700 ease-in-out z-10 ${
              isRightPanelActive ? 'translate-x-0' : '-translate-x-4'
            }`}>
              <h1 className="text-5xl font-bold mb-6">С возвращением!</h1>
              <p className="text-base font-light leading-7 tracking-wide my-5 text-white/80 max-w-md">
                Войдите, чтобы продолжить работу с системой анализа поведения и антифрода
              </p>
              <button
                type="button"
                onClick={() => setIsRightPanelActive(false)}
                className="mt-6 px-10 py-4 rounded-full border border-white/30 bg-white/5 hover:bg-white/10 text-white font-semibold transition-all active:scale-95 backdrop-blur-sm"
              >
                Войти
              </button>
            </div>

            {/* Right panel — shown when login is active */}
            <div className={`absolute flex items-center justify-center flex-col px-10 text-center top-0 h-full w-1/2 right-0 transition-transform duration-700 ease-in-out z-10 ${
              isRightPanelActive ? 'translate-x-4' : 'translate-x-0'
            }`}>
              <h1 className="text-5xl font-bold mb-6">Привет, друг!</h1>
              <p className="text-base font-light leading-7 tracking-wide my-5 text-white/80 max-w-md">
                Присоединяйтесь к ATdot — начните отслеживать фрод, анализировать поведение и строить когнитивный антифрод нового поколения
              </p>
              <button
                type="button"
                onClick={() => setIsRightPanelActive(true)}
                className="mt-6 px-10 py-4 rounded-full border border-white/30 bg-white/5 hover:bg-white/10 text-white font-semibold transition-all active:scale-95 backdrop-blur-sm"
              >
                Регистрация
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
