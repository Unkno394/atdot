'use client'
import { useEffect, useState } from 'react'
import { useRouter } from 'next/navigation'
import Link from 'next/link'
import { getStats, getRecentEvents, getApiKeys, getSessionScores,
         type Stats, type RecentEvent, type ApiKey, type SessionScores, type ScoreEntry } from '@/lib/api'
import AppShell from '@/components/AppShell'

const glass = {
  background: 'rgba(0,0,0,0.35)',
  backdropFilter: 'blur(16px)',
  border: '1px solid rgba(255,255,255,0.08)',
  borderRadius: 16,
} as const

function ActivityChart({ events }: { events: RecentEvent[] }) {
  const W = 480, H = 90, BAR = 28, PAD = 6
  const hours = Array.from({ length: 12 }, (_, i) => {
    const d = new Date(Date.now() - (11 - i) * 3_600_000)
    return { label: d.getHours().toString().padStart(2, '0'), count: 0 }
  })
  events.forEach(ev => {
    const h = new Date(ev.timestamp).getHours()
    const idx = hours.findIndex(x => parseInt(x.label) === h)
    if (idx >= 0) hours[idx].count++
  })
  const max = Math.max(...hours.map(h => h.count), 1)

  return (
    <svg width="100%" viewBox={`0 0 ${W} ${H + 20}`} preserveAspectRatio="none">
      {hours.map((h, i) => {
        const bh   = Math.max((h.count / max) * H, h.count > 0 ? 4 : 2)
        const x    = i * (BAR + PAD) + 4
        const fill = h.count > 0 ? '#7c3aed' : 'rgba(255,255,255,0.06)'
        return (
          <g key={i}>
            <rect x={x} y={H - bh} width={BAR} height={bh} rx={5} fill={fill} opacity={0.9} />
            {h.count > 0 && (
              <text x={x + BAR / 2} y={H - bh - 5} textAnchor="middle" fontSize={9} fill="#a78bfa">
                {h.count}
              </text>
            )}
            <text x={x + BAR / 2} y={H + 14} textAnchor="middle" fontSize={9} fill="rgba(255,255,255,0.25)">
              {h.label}
            </text>
          </g>
        )
      })}
    </svg>
  )
}

function DonutChart({ events }: { events: RecentEvent[] }) {
  const palette: Record<string, string> = {
    page_view: '#7c3aed', click: '#61dca3',
    purchase: '#61b3dc',  page_hide: '#6B7A99',
  }
  const counts: Record<string, number> = {}
  events.forEach(ev => { counts[ev.event_type] = (counts[ev.event_type] || 0) + 1 })
  const total  = events.length || 1
  const entries = Object.entries(counts)

  const R = 44, r = 26, cx = 60, cy = 60

  function arc(startPct: number, endPct: number) {
    const toXY = (pct: number) => {
      const a = (pct * 360 - 90) * (Math.PI / 180)
      return { x: cx + R * Math.cos(a), y: cy + R * Math.sin(a) }
    }
    const s  = toXY(startPct)
    const e  = toXY(endPct)
    const lg = endPct - startPct > 0.5 ? 1 : 0
    return `M ${cx} ${cy} L ${s.x} ${s.y} A ${R} ${R} 0 ${lg} 1 ${e.x} ${e.y} Z`
  }

  let cum = 0
  const slices = entries.map(([type, cnt]) => {
    const pct   = cnt / total
    const start = cum; cum += pct
    return { type, cnt, pct, start, color: palette[type] ?? '#a78bfa' }
  })

  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 20 }}>
      <svg width={120} height={120} viewBox="0 0 120 120">
        {slices.map((s, i) => (
          <path key={i} d={arc(s.start, s.start + s.pct)} fill={s.color} opacity={0.85} />
        ))}
        <circle cx={cx} cy={cy} r={r} fill="rgba(0,0,0,0.6)" />
        <text x={cx} y={cy - 5}  textAnchor="middle" fontSize={11} fill="rgba(255,255,255,0.5)">всего</text>
        <text x={cx} y={cy + 10} textAnchor="middle" fontSize={16} fill="#fff" fontWeight="bold">{total}</text>
      </svg>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        {slices.map(s => (
          <div key={s.type} style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <div style={{ width: 8, height: 8, borderRadius: 2, background: s.color, flexShrink: 0 }} />
            <span style={{ fontSize: 12, color: 'rgba(255,255,255,0.6)' }}>{s.type}</span>
            <span style={{ fontSize: 12, color: '#fff', marginLeft: 'auto', paddingLeft: 12 }}>{s.cnt}</span>
          </div>
        ))}
      </div>
    </div>
  )
}

// ── Score detail modal ────────────────────────────────────────────────────────

const ACTION_COLOR: Record<string, string> = {
  allow:     '#61dca3',
  challenge: '#fbbf24',
  block:     '#f87171',
}

function ScoreBar({ label, value, max = 1 }: { label: string; value: number | null; max?: number }) {
  if (value === null || value === undefined) return null
  const pct = Math.round((value / max) * 100)
  const color = value > 0.65 ? '#f87171' : value > 0.4 ? '#fbbf24' : '#61dca3'
  return (
    <div style={{ marginBottom: 10 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12, marginBottom: 4 }}>
        <span style={{ color: 'rgba(255,255,255,0.5)' }}>{label}</span>
        <span style={{ color, fontWeight: 600, fontFamily: 'monospace' }}>{value.toFixed(3)}</span>
      </div>
      <div style={{ height: 4, background: 'rgba(255,255,255,0.08)', borderRadius: 2 }}>
        <div style={{ height: '100%', width: `${pct}%`, background: color, borderRadius: 2, transition: 'width 0.3s' }} />
      </div>
    </div>
  )
}

function ScoreModal({ sessionId, onClose }: { sessionId: string; onClose: () => void }) {
  const [data, setData] = useState<SessionScores | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    getSessionScores(sessionId).then(setData).finally(() => setLoading(false))
  }, [sessionId])

  return (
    <div
      onClick={onClose}
      style={{
        position: 'fixed', inset: 0, zIndex: 1000,
        background: 'rgba(0,0,0,0.7)', backdropFilter: 'blur(4px)',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        padding: 24,
      }}
    >
      <div
        onClick={e => e.stopPropagation()}
        style={{
          background: 'rgba(12,12,16,0.97)', border: '1px solid rgba(255,255,255,0.1)',
          borderRadius: 16, padding: '28px 32px', width: '100%', maxWidth: 640,
          maxHeight: '80vh', overflowY: 'auto',
          boxShadow: '0 24px 64px rgba(0,0,0,0.6)',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
          <div>
            <div style={{ fontSize: 16, fontWeight: 700 }}>Анализ сессии</div>
            <div style={{ fontSize: 11, color: 'rgba(255,255,255,0.35)', fontFamily: 'monospace', marginTop: 4 }}>
              {sessionId}
            </div>
          </div>
          <button
            onClick={onClose}
            style={{
              background: 'none', border: '1px solid rgba(255,255,255,0.12)',
              borderRadius: 8, color: 'rgba(255,255,255,0.5)', cursor: 'pointer',
              padding: '6px 12px', fontSize: 13,
            }}
          >
            закрыть
          </button>
        </div>

        {loading && (
          <div style={{ textAlign: 'center', padding: 40, color: 'rgba(255,255,255,0.3)' }}>загрузка…</div>
        )}

        {!loading && (!data || data.scores.length === 0) && (
          <div style={{ textAlign: 'center', padding: 40, color: 'rgba(255,255,255,0.3)' }}>
            Нет данных скоринга для этой сессии
          </div>
        )}

        {data?.scores.map((s, i) => (
          <div key={s.id} style={{
            marginBottom: 20, padding: '18px 20px',
            background: 'rgba(255,255,255,0.03)', border: '1px solid rgba(255,255,255,0.07)',
            borderRadius: 12,
          }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 16 }}>
              <span style={{
                background: `${ACTION_COLOR[s.action] ?? '#6B7A99'}22`,
                color: ACTION_COLOR[s.action] ?? '#6B7A99',
                padding: '3px 10px', borderRadius: 6,
                fontWeight: 700, fontSize: 11, letterSpacing: '0.06em',
              }}>
                {s.action.toUpperCase()}
              </span>
              {s.event_type && (
                <span style={{ fontSize: 12, color: 'rgba(255,255,255,0.4)' }}>{s.event_type}</span>
              )}
              <span style={{ fontSize: 11, color: 'rgba(255,255,255,0.25)', marginLeft: 'auto' }}>
                {new Date(s.timestamp).toLocaleTimeString('ru-RU')}
              </span>
            </div>

            <div style={{ marginBottom: 16 }}>
              <ScoreBar label="Итоговый score"  value={s.score} />
              <ScoreBar label="L1 — поведение"  value={s.l1} />
              <ScoreBar label="L2 — сеть/устройство" value={s.l2} />
              <ScoreBar label="L3 — паттерны"   value={s.l3} />
              <ScoreBar label="Embedding drift"  value={s.embedding} />
            </div>

            {s.reasons.length > 0 && (
              <div>
                <div style={{ fontSize: 11, fontWeight: 600, color: 'rgba(255,255,255,0.35)', marginBottom: 8, textTransform: 'uppercase', letterSpacing: '0.08em' }}>
                  Причины
                </div>
                {s.reasons.map((r, j) => (
                  <div key={j} style={{
                    fontSize: 12, color: 'rgba(255,255,255,0.7)',
                    padding: '5px 10px', marginBottom: 4,
                    background: 'rgba(248,113,113,0.06)',
                    border: '1px solid rgba(248,113,113,0.15)',
                    borderRadius: 6, lineHeight: 1.5,
                  }}>
                    {r}
                  </div>
                ))}
              </div>
            )}

            {s.reasons.length === 0 && (
              <div style={{ fontSize: 12, color: 'rgba(255,255,255,0.25)', fontStyle: 'italic' }}>
                Подозрительных паттернов не обнаружено
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  )
}

function StatCard({ label, value, accent, description }: { label: string; value: number | string; accent?: string; description?: string }) {
  const [tip, setTip] = useState(false)
  return (
    <div style={{ ...glass, padding: '22px 24px', position: 'relative' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 14 }}>
        <div style={{ fontSize: 11, fontWeight: 600, color: 'rgba(255,255,255,0.4)', textTransform: 'uppercase', letterSpacing: '0.08em' }}>
          {label}
        </div>
        {description && (
          <div style={{ position: 'relative', display: 'inline-flex' }}
            onMouseEnter={() => setTip(true)}
            onMouseLeave={() => setTip(false)}>
            <span style={{
              fontSize: 10, color: 'rgba(255,255,255,0.3)', cursor: 'help',
              border: '1px solid rgba(255,255,255,0.15)', borderRadius: '50%',
              width: 15, height: 15, display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0,
            }}>?</span>
            {tip && (
              <div style={{
                position: 'absolute', bottom: 'calc(100% + 8px)', left: '50%',
                transform: 'translateX(-50%)',
                background: 'rgba(10,10,10,0.97)', border: '1px solid rgba(255,255,255,0.12)',
                borderRadius: 10, padding: '10px 14px', fontSize: 12,
                color: 'rgba(255,255,255,0.75)', width: 220, zIndex: 50,
                lineHeight: 1.6, boxShadow: '0 8px 32px rgba(0,0,0,0.5)',
                pointerEvents: 'none',
              }}>
                {description}
              </div>
            )}
          </div>
        )}
      </div>
      <div style={{ fontSize: 38, fontWeight: 700, letterSpacing: '-0.03em', color: accent ?? '#fff' }}>
        {value}
      </div>
    </div>
  )
}

const TYPE_COLOR: Record<string, string> = {
  page_view: '#6B7A99', click: '#a78bfa', purchase: '#61dca3', page_hide: '#6B7A99',
}

function EventRow({ ev, onClick }: { ev: RecentEvent; onClick: () => void }) {
  const [hovered, setHovered] = useState(false)
  const color = TYPE_COLOR[ev.event_type] ?? '#fbbf24'
  const time  = new Date(ev.timestamp).toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit', second: '2-digit' })
  return (
    <div
      onClick={onClick}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      style={{
        display: 'grid', gridTemplateColumns: '140px 1fr 120px 90px 24px',
        padding: '12px 20px', borderBottom: '1px solid rgba(255,255,255,0.05)',
        fontSize: 13, alignItems: 'center', gap: 16,
        cursor: 'pointer',
        background: hovered ? 'rgba(255,255,255,0.03)' : 'transparent',
        transition: 'background 0.12s',
      }}
    >
      <span style={{
        background: `${color}22`, color, padding: '3px 10px',
        borderRadius: 6, fontWeight: 600, fontSize: 11, textAlign: 'center', letterSpacing: '0.04em',
      }}>
        {ev.event_type}
      </span>
      <span style={{ color: 'rgba(255,255,255,0.35)', fontFamily: 'monospace', fontSize: 11 }}>
        {ev.session_id.slice(0, 18)}…
      </span>
      <span style={{ color: 'rgba(255,255,255,0.35)', fontSize: 12 }}>{ev.ip || '—'}</span>
      <span style={{ color: 'rgba(255,255,255,0.35)', fontSize: 12, textAlign: 'right' }}>{time}</span>
      <span style={{ color: 'rgba(255,255,255,0.2)', fontSize: 14 }}>›</span>
    </div>
  )
}

type WsEvent = {
  type:       'new_event'
  session_id: string
  event_type: string
  ip:         string | null
  timestamp:  string
  score:      number
  action:     string
  reasons:    string[]
}

function useRealtimeDashboard(
  setStats:   (s: Stats) => void,
  setEvents:  React.Dispatch<React.SetStateAction<RecentEvent[]>>,
  setOnline:  (v: boolean) => void,
) {
  useEffect(() => {
    const token = localStorage.getItem('mdr_token')
    if (!token) return

    const base = (process.env.NEXT_PUBLIC_API_URL ?? 'http://localhost:8080')
      .replace(/^https/, 'wss').replace(/^http/, 'ws')

    let ws: WebSocket
    let dead = false
    let retryMs = 1000

    function connect() {
      if (dead) return
      ws = new WebSocket(`${base}/ws?token=${token}`)

      ws.onopen  = () => { retryMs = 1000; setOnline(true) }

      ws.onmessage = (e) => {
        try {
          const msg: WsEvent = JSON.parse(e.data)
          if (msg.type !== 'new_event') return

          // Prepend new event to the list
          setEvents(prev => [{
            id:         crypto.randomUUID(),
            session_id: msg.session_id,
            event_type: msg.event_type,
            ip:         msg.ip,
            timestamp:  msg.timestamp,
          }, ...prev].slice(0, 50))

          // Refresh stats silently after each event
          getStats().then(setStats).catch(() => {})
        } catch {}
      }

      ws.onclose = () => {
        setOnline(false)
        if (dead) return
        setTimeout(connect, retryMs)
        retryMs = Math.min(retryMs * 2, 30_000)
      }
    }

    connect()
    return () => { dead = true; ws?.close() }
  }, [setStats, setEvents, setOnline])
}

export default function Dashboard() {
  const router = useRouter()
  const [stats,           setStats]           = useState<Stats | null>(null)
  const [events,          setEvents]          = useState<RecentEvent[]>([])
  const [keys,            setKeys]            = useState<ApiKey[]>([])
  const [error,           setError]           = useState('')
  const [online,          setOnline]          = useState(false)
  const [selectedSession, setSelectedSession] = useState<string | null>(null)

  useEffect(() => {
    if (!localStorage.getItem('mdr_token')) { router.push('/auth/login'); return }
    Promise.all([getStats(), getRecentEvents(), getApiKeys()])
      .then(([s, e, k]) => { setStats(s); setEvents(e); setKeys(k) })
      .catch(e => setError(e.message))
  }, [router])

  useRealtimeDashboard(setStats, setEvents, setOnline)

  return (
    <AppShell>
      <div style={{ display: 'flex', alignItems: 'center', gap: 14, marginBottom: 6 }}>
        <h1 style={{ fontSize: 26, fontWeight: 700, letterSpacing: '-0.03em', margin: 0 }}>Обзор</h1>
        <span style={{
          display: 'flex', alignItems: 'center', gap: 5,
          fontSize: 11, fontWeight: 600, letterSpacing: '0.06em',
          color: online ? '#61dca3' : 'rgba(255,255,255,0.25)',
          background: online ? 'rgba(97,220,163,0.1)' : 'rgba(255,255,255,0.05)',
          border: `1px solid ${online ? 'rgba(97,220,163,0.25)' : 'rgba(255,255,255,0.08)'}`,
          borderRadius: 20, padding: '3px 10px', transition: 'all 0.4s',
        }}>
          <span style={{
            width: 6, height: 6, borderRadius: '50%',
            background: online ? '#61dca3' : 'rgba(255,255,255,0.2)',
            boxShadow: online ? '0 0 6px #61dca3' : 'none',
            animation: online ? 'pulse 2s infinite' : 'none',
          }} />
          {online ? 'LIVE' : 'offline'}
        </span>
      </div>
      <p style={{ color: 'rgba(255,255,255,0.4)', fontSize: 14, marginBottom: 36 }}>Данные за последние 24 часа</p>

      {error && (
        <div style={{ ...glass, padding: '12px 16px', color: '#f87171', marginBottom: 24, fontSize: 14, borderColor: 'rgba(248,113,113,0.2)' }}>
          {error}
        </div>
      )}

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 16, marginBottom: 28 }}>
        <StatCard label="DAU" value={stats?.dau ?? '—'} description="Daily Active Users — сколько уникальных пользователей было активно сегодня. Считается по уникальным session_id за последние 24 часа." />
        <StatCard label="Событий сегодня" value={stats?.events_today ?? '—'} description="Сумма всех событий (клики, просмотры, покупки и т.д.), зафиксированных SDK за сегодня." />
        <StatCard label="Сессий всего" value={stats?.total_sessions ?? '—'} description="Общее количество сессий за всё время. Каждая сессия — один непрерывный визит пользователя на ваш сайт." />
        <StatCard label="Фрод-алертов" value={stats?.fraud_alerts ?? '—'} accent="#f87171" description="Количество сессий, которые система пометила как подозрительные: боты, угнанные аккаунты, аномальное поведение." />
      </div>

      <div style={{ ...glass, padding: '22px 28px', marginBottom: 28, display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
          <div style={{
            width: 40, height: 40, borderRadius: 10, background: 'rgba(124,58,237,0.2)',
            border: '1px solid rgba(124,58,237,0.3)', display: 'flex', alignItems: 'center', justifyContent: 'center',
            fontSize: 18,
          }}>⬡</div>
          <div>
            <div style={{ fontSize: 15, fontWeight: 600, marginBottom: 2 }}>
              {keys.length === 0 ? 'Нет активных ключей' : `${keys.length} API ${keys.length === 1 ? 'ключ' : keys.length < 5 ? 'ключа' : 'ключей'}`}
            </div>
            <div style={{ fontSize: 12, color: 'rgba(255,255,255,0.4)' }}>
              {keys.length === 0
                ? 'Создайте ключ на странице Интеграция, чтобы начать сбор данных'
                : `Последний создан: ${new Date(keys[0]?.created_at ?? '').toLocaleDateString('ru-RU')}`
              }
            </div>
          </div>
        </div>
        <Link href="/keys" style={{
          background: 'rgba(124,58,237,0.2)', border: '1px solid rgba(124,58,237,0.3)',
          color: '#a78bfa', borderRadius: 10, padding: '9px 18px',
          fontSize: 13, fontWeight: 600, textDecoration: 'none', transition: 'all 0.15s',
        }}>
          {keys.length === 0 ? 'Создать ключ →' : 'Управление →'}
        </Link>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 280px', gap: 16, marginBottom: 28 }}>
        <div style={{ ...glass, padding: '24px 28px' }}>
          <div style={{ fontSize: 13, fontWeight: 600, color: 'rgba(255,255,255,0.5)', marginBottom: 20, textTransform: 'uppercase', letterSpacing: '0.06em' }}>
            Активность по часам
          </div>
          <ActivityChart events={events} />
        </div>

        <div style={{ ...glass, padding: '24px 28px' }}>
          <div style={{ fontSize: 13, fontWeight: 600, color: 'rgba(255,255,255,0.5)', marginBottom: 20, textTransform: 'uppercase', letterSpacing: '0.06em' }}>
            Типы событий
          </div>
          <DonutChart events={events} />
        </div>
      </div>

      <div style={{ ...glass, overflow: 'hidden' }}>
        <div style={{
          padding: '14px 20px', borderBottom: '1px solid rgba(255,255,255,0.06)',
          display: 'grid', gridTemplateColumns: '140px 1fr 120px 90px 24px',
          gap: 16, fontSize: 10, fontWeight: 700,
          color: 'rgba(255,255,255,0.3)', textTransform: 'uppercase', letterSpacing: '0.08em',
        }}>
          <span>Событие</span><span>Сессия</span><span>IP</span>
          <span style={{ textAlign: 'right' }}>Время</span><span />
        </div>
        {events.length === 0 ? (
          <div style={{ padding: '48px 32px', textAlign: 'center' }}>
            <div style={{ fontSize: 32, marginBottom: 16 }}>📡</div>
            <div style={{ color: 'rgba(255,255,255,0.5)', fontSize: 15, fontWeight: 500, marginBottom: 8 }}>Событий пока нет</div>
            <div style={{ color: 'rgba(255,255,255,0.25)', fontSize: 13, lineHeight: 1.6, maxWidth: 360, margin: '0 auto' }}>
              Подключите SDK на странице <Link href="/keys" style={{ color: '#a78bfa', textDecoration: 'none' }}>Интеграция</Link> — после первого события данные появятся здесь автоматически
            </div>
          </div>
        ) : (
          events.map(ev => (
            <EventRow
              key={ev.id}
              ev={ev}
              onClick={() => setSelectedSession(ev.session_id)}
            />
          ))
        )}
      </div>

      {selectedSession && (
        <ScoreModal
          sessionId={selectedSession}
          onClose={() => setSelectedSession(null)}
        />
      )}
    </AppShell>
  )
}
