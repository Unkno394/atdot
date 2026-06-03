'use client'

import Link from 'next/link'
import { useState, useEffect } from 'react'
import LetterGlitch from '@/components/LetterGlitch'

const SNIPPET = `<script
  src="https://api.atdot.ru/sdk/atdot.js"
  data-key="YOUR_API_KEY">
</script>`

const FEATURES = [
  {
    icon: '⬡',
    title: 'Строит профиль каждого пользователя',
    desc: 'Система запоминает, как обычно ведёт себя человек — по каким страницам ходит, в каком порядке, с какой скоростью.',
  },
  {
    icon: '◈',
    title: 'Замечает необычное поведение',
    desc: 'Аккаунт угнали? Бот пытается купить подписку? Система видит отклонение от нормы и бьёт тревогу.',
  },
  {
    icon: '◎',
    title: 'Объясняет почему',
    desc: 'Не просто "фрод", а "пользователь перешёл к оплате в 3 раза быстрее обычного" или "мышь двигалась по прямой линии".',
  },
]

const glassCard = {
  background: 'rgba(0, 0, 0, 0.2)',
  backdropFilter: 'blur(10px)',
  borderRadius: 24,
  padding: 32,
  transition: 'all 0.2s ease',
  border: 'none',
}

export default function Landing() {
  const [isLoggedIn, setIsLoggedIn] = useState(false)
  const [mounted, setMounted] = useState(false)
  useEffect(() => {
    setMounted(true)
    setIsLoggedIn(!!localStorage.getItem('mdr_token'))
  }, [])

  return (
    <div className="relative min-h-screen overflow-hidden" style={{ background: 'var(--bg)' }}>
      
      <div className="fixed inset-0 -z-10">
        <LetterGlitch
          glitchColors={['#2b4539', '#61dca3', '#61b3dc']}
          glitchSpeed={150}
          centerVignette={false}
          outerVignette={true}
          smooth={true}
          characters="ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
        />
        <div className="absolute inset-0 bg-black/70 pointer-events-none" />
      </div>

      <div style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        padding: '24px 48px',
        zIndex: 50,
        background: 'transparent',
      }}>
        <span style={{ fontWeight: 700, fontSize: 20, letterSpacing: '-0.02em', color: '#fff' }}>
          <span style={{ color: '#7c3aed' }}>AT</span>dot
        </span>
        {mounted && isLoggedIn ? (
          <Link
            href="/dashboard"
            style={{
              background: 'rgba(76, 29, 149, 0.35)',
              backdropFilter: 'blur(10px)',
              color: '#fff',
              padding: '10px 24px',
              borderRadius: 100,
              textDecoration: 'none',
              fontSize: 14,
              fontWeight: 500,
              transition: '0.2s',
            }}
          >
            Личный кабинет →
          </Link>
        ) : (
          <Link
            href="/auth/register"
            style={{
              background: 'rgba(76, 29, 149, 0.35)',
              backdropFilter: 'blur(10px)',
              color: '#fff',
              padding: '10px 24px',
              borderRadius: 100,
              textDecoration: 'none',
              fontSize: 14,
              fontWeight: 500,
              transition: '0.2s',
            }}
          >
            Зарегистрироваться / Войти
          </Link>
        )}
      </div>

      <section style={{ padding: '180px 48px 80px', maxWidth: 800, margin: '0 auto', textAlign: 'center' }}>
        <h1 style={{
          fontSize: 'clamp(42px, 6vw, 76px)', fontWeight: 700,
          lineHeight: 1.05, letterSpacing: '-0.03em', marginBottom: 24,
        }}>
          Ловите фрод.{' '}
          <span style={{ color: '#ffffff' }}>Объясняем почему.</span>
        </h1>

        <p style={{ fontSize: 18, color: 'rgba(255, 255, 255, 0.7)', lineHeight: 1.7, marginBottom: 48 }}>
          ATdot анализирует каждое движение пользователя: куда кликнул, как вёл мышь, сколько паузил. 
          Боты и угонщики не проходят.
        </p>

        <div style={{ display: 'flex', gap: 16, justifyContent: 'center', flexWrap: 'wrap' }}>
          <Link
            href={mounted && isLoggedIn ? '/dashboard' : '/auth/register'}
            style={{
              background: 'rgba(76, 29, 149, 0.4)',
              backdropFilter: 'blur(10px)',
              color: '#fff',
              padding: '14px 36px',
              borderRadius: 100,
              textDecoration: 'none',
              fontWeight: 600,
              fontSize: 16,
              transition: '0.2s',
            }}
          >
            {mounted && isLoggedIn ? 'Личный кабинет →' : 'Создать аккаунт'}
          </Link>
          <a
            href="#integration"
            style={{
              background: 'rgba(0, 0, 0, 0.3)',
              backdropFilter: 'blur(10px)',
              color: 'rgba(255, 255, 255, 0.85)',
              padding: '14px 36px',
              borderRadius: 100,
              textDecoration: 'none',
              fontWeight: 500,
              fontSize: 16,
              transition: '0.2s',
            }}
          >
            Как подключить
          </a>
        </div>
      </section>

      <section style={{ padding: '80px 48px', maxWidth: 1100, margin: '0 auto' }}>
        <div style={{
          display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))', gap: 24,
        }}>
          {FEATURES.map((f) => (
            <div key={f.title} style={glassCard}>
              <div style={{ fontSize: 32, marginBottom: 16, color: '#a78bfa' }}>{f.icon}</div>
              <h3 style={{ fontWeight: 600, marginBottom: 12, fontSize: 18, color: '#fff' }}>{f.title}</h3>
              <p style={{ color: 'rgba(255, 255, 255, 0.6)', fontSize: 14, lineHeight: 1.65 }}>{f.desc}</p>
            </div>
          ))}
        </div>
      </section>

      <section id="integration" style={{ padding: '80px 48px', maxWidth: 800, margin: '0 auto' }}>
        <h2 style={{ fontSize: 32, fontWeight: 700, marginBottom: 12, letterSpacing: '-0.02em', color: '#fff' }}>
          Подключение за 30 секунд
        </h2>
        <p style={{ color: 'rgba(255, 255, 255, 0.6)', marginBottom: 32, fontSize: 15 }}>
          Вставьте один тег — ATdot сам начнёт собирать события.
        </p>

        <div style={{
          background: 'rgba(0, 0, 0, 0.2)',
          backdropFilter: 'blur(10px)',
          borderRadius: 24,
          overflow: 'hidden',
          border: '1px solid rgba(255, 255, 255, 0.05)',
        }}>
          <div style={{
            padding: '12px 20px',
            borderBottom: '1px solid rgba(255, 255, 255, 0.05)',
            display: 'flex',
            gap: 8,
          }}>
            <div style={{ width: 12, height: 12, borderRadius: '50%', background: '#F87171' }} />
            <div style={{ width: 12, height: 12, borderRadius: '50%', background: '#FBBF24' }} />
            <div style={{ width: 12, height: 12, borderRadius: '50%', background: '#34D399' }} />
          </div>
          <pre style={{
            padding: '24px',
            fontSize: 13,
            lineHeight: 1.8,
            color: 'rgba(255, 255, 255, 0.8)',
            overflowX: 'auto',
            margin: 0,
            background: 'rgba(0, 0, 0, 0.4)',
          }}>
            <code>{SNIPPET}</code>
          </pre>
        </div>

        <p style={{ color: 'rgba(255, 255, 255, 0.5)', fontSize: 13, marginTop: 16 }}>
          Для трекинга кастомных событий: <code style={{ color: '#a78bfa' }}>ATdot.track('purchase', {'{ amount: 99 }'})</code>
        </p>
      </section>

      <footer style={{
        padding: '32px 48px',
        color: 'rgba(255, 255, 255, 0.5)',
        fontSize: 13,
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        background: 'rgba(0, 0, 0, 0.3)',
        backdropFilter: 'blur(10px)',
        marginTop: 60,
        borderTop: '1px solid rgba(255, 255, 255, 0.05)',
      }}>
        <span><span style={{ color: '#a78bfa' }}>AT</span>dot © 2025</span>
        <span>Написано на Rust + Next.js</span>
      </footer>
    </div>
  )
}