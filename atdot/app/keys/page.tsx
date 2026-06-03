'use client'
import { useEffect, useState } from 'react'
import { useRouter } from 'next/navigation'
import { getApiKeys, createApiKey, deleteApiKey, type ApiKey } from '@/lib/api'
import AppShell from '@/components/AppShell'
import Modal from '@/components/Modal'

const glass = {
  background: 'rgba(0,0,0,0.35)',
  backdropFilter: 'blur(16px)',
  border: '1px solid rgba(255,255,255,0.08)',
  borderRadius: 16,
} as const

const SNIPPET = (key: string) => `<script
  src="https://api.atdot.ru/sdk/atdot.js"
  data-key="${key}">
</script>`

export default function KeysPage() {
  const router = useRouter()
  const [keys,      setKeys]      = useState<ApiKey[]>([])
  const [name,      setName]      = useState('')
  const [copied,    setCopied]    = useState<string | null>(null)
  const [loading,   setLoading]   = useState(false)
  const [error,     setError]     = useState('')
  const [howToOpen, setHowToOpen] = useState(false)

  useEffect(() => {
    if (!localStorage.getItem('mdr_token')) { router.push('/auth/login'); return }
    getApiKeys().then(setKeys).catch(e => setError(e.message))
  }, [router])

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault()
    if (!name.trim()) return
    setLoading(true); setError('')
    try {
      const k = await createApiKey(name.trim())
      setKeys(prev => [k, ...prev])
      setName('')
    } catch (e: any) { setError(e.message) }
    finally { setLoading(false) }
  }

  async function handleDelete(id: string) {
    await deleteApiKey(id)
    setKeys(prev => prev.filter(k => k.id !== id))
  }

  function copy(text: string, id: string) {
    navigator.clipboard.writeText(text)
    setCopied(id)
    setTimeout(() => setCopied(null), 2000)
  }

  return (
    <AppShell>
      <h1 style={{ fontSize: 26, fontWeight: 700, letterSpacing: '-0.03em', marginBottom: 6 }}>Интеграция</h1>
      <div style={{ display: 'flex', alignItems: 'center', gap: 16, marginBottom: 36 }}>
        <p style={{ color: 'rgba(255,255,255,0.4)', fontSize: 14, margin: 0 }}>Управление API ключами и подключение SDK</p>
        <button
          onClick={() => setHowToOpen(true)}
          style={{
            background: 'rgba(124,58,237,0.2)', border: '1px solid rgba(124,58,237,0.3)',
            color: '#a78bfa', borderRadius: 8, padding: '6px 14px',
            fontSize: 13, fontWeight: 600, cursor: 'pointer', transition: 'all 0.15s',
          }}
        >
          Как подключить?
        </button>
      </div>

      {error && (
        <div style={{ ...glass, padding: '12px 16px', color: '#f87171', marginBottom: 24, fontSize: 14, borderColor: 'rgba(248,113,113,0.2)' }}>
          {error}
        </div>
      )}

      {/* create key */}
      <div style={{ ...glass, padding: '28px 32px', marginBottom: 24 }}>
        <div style={{ fontSize: 13, fontWeight: 700, color: 'rgba(255,255,255,0.5)', textTransform: 'uppercase', letterSpacing: '0.08em', marginBottom: 20 }}>
          Новый ключ
        </div>
        <form onSubmit={handleCreate} style={{ display: 'flex', gap: 12 }}>
          <input
            value={name}
            onChange={e => setName(e.target.value)}
            placeholder="Название, например: production"
            style={{
              flex: 1, background: 'rgba(255,255,255,0.06)',
              border: '1px solid rgba(255,255,255,0.1)',
              borderRadius: 10, padding: '11px 16px',
              color: '#fff', fontSize: 14, outline: 'none',
            }}
          />
          <button
            type="submit" disabled={loading || !name.trim()}
            style={{
              background: loading || !name.trim() ? 'rgba(124,58,237,0.25)' : 'rgba(124,58,237,0.5)',
              border: '1px solid rgba(124,58,237,0.4)',
              color: '#fff', borderRadius: 10, padding: '11px 28px',
              fontWeight: 600, fontSize: 14, cursor: loading || !name.trim() ? 'default' : 'pointer',
              backdropFilter: 'blur(8px)', transition: 'all 0.15s',
            }}
          >
            {loading ? '...' : 'Создать'}
          </button>
        </form>
      </div>

      {/* keys list */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
        {keys.length === 0 ? (
          <div style={{ ...glass, padding: 48, textAlign: 'center', color: 'rgba(255,255,255,0.25)', fontSize: 14 }}>
            Ключей пока нет — создайте первый выше
          </div>
        ) : keys.map(k => (
          <div key={k.id} style={{ ...glass, padding: '24px 28px' }}>
            {/* header */}
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 20 }}>
              <span style={{ fontWeight: 700, fontSize: 16 }}>{k.name}</span>
              <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
                <span style={{ color: 'rgba(255,255,255,0.3)', fontSize: 12 }}>
                  {new Date(k.created_at).toLocaleDateString('ru-RU')}
                </span>
                <button
                  onClick={() => handleDelete(k.id)}
                  style={{
                    background: 'rgba(248,113,113,0.1)', border: '1px solid rgba(248,113,113,0.2)',
                    color: '#f87171', borderRadius: 8, padding: '5px 12px',
                    fontSize: 12, cursor: 'pointer',
                  }}
                >
                  Удалить
                </button>
              </div>
            </div>

            {/* api key */}
            <div style={{ marginBottom: 16 }}>
              <div style={{ fontSize: 11, fontWeight: 600, color: 'rgba(255,255,255,0.3)', textTransform: 'uppercase', letterSpacing: '0.07em', marginBottom: 8 }}>
                API Key
              </div>
              <div style={{
                background: 'rgba(124,58,237,0.1)', border: '1px solid rgba(124,58,237,0.2)',
                borderRadius: 10, padding: '12px 16px',
                display: 'flex', justifyContent: 'space-between', alignItems: 'center',
              }}>
                <code style={{ fontSize: 13, color: '#a78bfa', letterSpacing: '0.02em' }}>{k.key}</code>
                <button
                  onClick={() => copy(k.key, k.id + '_key')}
                  style={{
                    background: copied === k.id + '_key' ? 'rgba(97,220,163,0.15)' : 'rgba(255,255,255,0.06)',
                    border: '1px solid rgba(255,255,255,0.1)',
                    color: copied === k.id + '_key' ? '#61dca3' : 'rgba(255,255,255,0.5)',
                    borderRadius: 6, padding: '4px 12px', fontSize: 12, cursor: 'pointer',
                    transition: 'all 0.15s',
                  }}
                >
                  {copied === k.id + '_key' ? '✓ скопировано' : 'копировать'}
                </button>
              </div>
            </div>

            {/* snippet */}
            <div>
              <div style={{ fontSize: 11, fontWeight: 600, color: 'rgba(255,255,255,0.3)', textTransform: 'uppercase', letterSpacing: '0.07em', marginBottom: 8 }}>
                HTML snippet
              </div>
              <div style={{ background: 'rgba(0,0,0,0.4)', border: '1px solid rgba(255,255,255,0.06)', borderRadius: 10, overflow: 'hidden' }}>
                <div style={{ display: 'flex', justifyContent: 'flex-end', padding: '8px 14px', borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
                  <button
                    onClick={() => copy(SNIPPET(k.key), k.id + '_snip')}
                    style={{
                      background: copied === k.id + '_snip' ? 'rgba(97,220,163,0.15)' : 'rgba(255,255,255,0.06)',
                      border: '1px solid rgba(255,255,255,0.1)',
                      color: copied === k.id + '_snip' ? '#61dca3' : 'rgba(255,255,255,0.5)',
                      borderRadius: 6, padding: '4px 12px', fontSize: 12, cursor: 'pointer',
                    }}
                  >
                    {copied === k.id + '_snip' ? '✓ скопировано' : 'копировать'}
                  </button>
                </div>
                <pre style={{ padding: '16px 20px', fontSize: 13, lineHeight: 1.8, margin: 0, overflowX: 'auto', color: 'rgba(255,255,255,0.7)' }}>
                  <code>{SNIPPET(k.key)}</code>
                </pre>
              </div>
            </div>
          </div>
        ))}
      </div>

      <Modal open={howToOpen} onClose={() => setHowToOpen(false)} title="Как подключить ATdot">
        <div style={{ display: 'flex', flexDirection: 'column', gap: 20, fontSize: 14, color: 'rgba(255,255,255,0.8)' }}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            <div style={{ fontWeight: 600, color: '#fff', fontSize: 15 }}>1. Создайте API ключ</div>
            <div style={{ color: 'rgba(255,255,255,0.5)', lineHeight: 1.6 }}>Введите название ключа выше и нажмите «Создать». Каждый проект должен иметь свой ключ.</div>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            <div style={{ fontWeight: 600, color: '#fff', fontSize: 15 }}>2. Вставьте скрипт на сайт</div>
            <div style={{ color: 'rgba(255,255,255,0.5)', lineHeight: 1.6 }}>Скопируйте HTML snippet из карточки ключа и вставьте его перед закрывающим тегом &lt;/body&gt; на вашем сайте.</div>
            <pre style={{ background: 'rgba(255,255,255,0.05)', borderRadius: 10, padding: '12px 16px', fontSize: 12, color: '#a78bfa', overflow: 'auto', margin: 0 }}>
{`<script
  src="https://api.atdot.ru/sdk/atdot.js"
  data-key="ВАШ_КЛЮЧ">
</script>`}
            </pre>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            <div style={{ fontWeight: 600, color: '#fff', fontSize: 15 }}>3. Кастомные события (опционально)</div>
            <div style={{ color: 'rgba(255,255,255,0.5)', lineHeight: 1.6 }}>Отправляйте собственные события с данными для более точного анализа поведения.</div>
            <pre style={{ background: 'rgba(255,255,255,0.05)', borderRadius: 10, padding: '12px 16px', fontSize: 12, color: '#61dca3', overflow: 'auto', margin: 0 }}>
{`ATdot.track('purchase', { amount: 99, currency: 'RUB' })
ATdot.track('login', { method: 'google' })`}
            </pre>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            <div style={{ fontWeight: 600, color: '#fff', fontSize: 15 }}>4. Готово!</div>
            <div style={{ color: 'rgba(255,255,255,0.5)', lineHeight: 1.6 }}>Данные начнут поступать в дашборд в течение нескольких минут после первого посещения пользователей.</div>
          </div>
        </div>
      </Modal>
    </AppShell>
  )
}
