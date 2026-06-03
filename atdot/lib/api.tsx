const BASE = process.env.NEXT_PUBLIC_API_URL ?? 'http://localhost:8080'

function getToken(): string | null {
  if (typeof window === 'undefined') return null
  return localStorage.getItem('mdr_token')
}

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const token = getToken()
  const res = await fetch(`${BASE}${path}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...(options.headers ?? {}),
    },
  })
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: 'unknown error' }))
    throw new Error(err.error ?? 'request failed')
  }
  return res.json()
}

// ── auth ──────────────────────────────────────────────────────
export interface AuthResult {
  token:      string
  email:      string
  session_id: string
}

export async function register(email: string, password: string): Promise<AuthResult> {
  return request<AuthResult>('/api/auth/register', {
    method: 'POST',
    body:   JSON.stringify({ email, password }),
  })
}

export async function login(email: string, password: string): Promise<AuthResult> {
  return request<AuthResult>('/api/auth/login', {
    method: 'POST',
    body:   JSON.stringify({ email, password }),
  })
}

export async function logout(): Promise<void> {
  await request('/api/auth/logout', { method: 'POST' })
}

export interface Me {
  user_id:    string
  email:      string
  session_id: string
}

export async function getMe(): Promise<Me> {
  return request<Me>('/api/auth/me')
}

export async function updateEmail(email: string): Promise<void> {
  await request('/api/auth/me', {
    method: 'PATCH',
    body:   JSON.stringify({ email }),
  })
}

export async function changePassword(currentPassword: string, newPassword: string): Promise<void> {
  await request('/api/auth/me', {
    method: 'PATCH',
    body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }),
  })
}

export function saveSession(result: AuthResult) {
  localStorage.setItem('mdr_token',      result.token)
  localStorage.setItem('mdr_email',      result.email)
  localStorage.setItem('mdr_session_id', result.session_id)
}

export function clearSession() {
  localStorage.removeItem('mdr_token')
  localStorage.removeItem('mdr_email')
  localStorage.removeItem('mdr_session_id')
}

// ── api keys ──────────────────────────────────────────────────
export interface ApiKey {
  id:         string
  key:        string
  name:       string
  created_at: string
}

export async function getApiKeys(): Promise<ApiKey[]> {
  return request('/api/keys')
}

export async function createApiKey(name: string): Promise<ApiKey> {
  return request('/api/keys', {
    method: 'POST',
    body:   JSON.stringify({ name }),
  })
}

export async function deleteApiKey(id: string) {
  return request(`/api/keys/${id}`, { method: 'DELETE' })
}

// ── stats ─────────────────────────────────────────────────────
export interface Stats {
  dau:            number
  events_today:   number
  total_sessions: number
  fraud_alerts:   number
}

export async function getStats(): Promise<Stats> {
  return request('/api/stats')
}

// ── events ────────────────────────────────────────────────────
export interface RecentEvent {
  id:         string
  session_id: string
  event_type: string
  timestamp:  string
  ip:         string | null
}

export async function getRecentEvents(): Promise<RecentEvent[]> {
  return request('/api/events/recent')
}
