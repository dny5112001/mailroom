// Mailroom web UI. Plain ES modules, no framework and no build step: the
// whole UI ships inside the binary.

const PAGE_SIZE = 100;
const DOCS_URL = 'https://github.com/dny5112001/mailroom#testing-with-the-api';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const $ = (selector, root = document) => root.querySelector(selector);

class Html {
  constructor(value) { this.value = value; }
  toString() { return this.value; }
}

const ESCAPES = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };
const escape = (value) => String(value).replace(/[&<>"']/g, (c) => ESCAPES[c]);
const raw = (value) => new Html(value);

function toHtml(value) {
  if (value === null || value === undefined || value === false) return '';
  if (value instanceof Html) return value.value;
  if (Array.isArray(value)) return value.map(toHtml).join('');
  return escape(value);
}

/** Tagged template that escapes every interpolated value unless it is `Html`. */
function html(strings, ...values) {
  let out = strings[0];
  for (let i = 0; i < values.length; i++) out += toHtml(values[i]) + strings[i + 1];
  return new Html(out);
}

const ICONS = {
  inbox: '<path d="M3.5 13.5h4.5l1.5 2.5h5l1.5-2.5h4.5"/><path d="M5.8 5.5h12.4l2.3 8v4.5a1.5 1.5 0 0 1-1.5 1.5h-14A1.5 1.5 0 0 1 3.5 18v-4.5l2.3-8Z"/>',
  unread: '<path d="M20.5 11v6.5a1.5 1.5 0 0 1-1.5 1.5H5a1.5 1.5 0 0 1-1.5-1.5v-11A1.5 1.5 0 0 1 5 5h9"/><path d="m3.8 6.5 8.2 6 4-2.9"/><circle cx="19" cy="6" r="2.5" fill="currentColor" stroke="none"/>',
  paperclip: '<path d="m20 11.5-7.8 7.8a4.6 4.6 0 0 1-6.5-6.5l7.9-7.9a3.1 3.1 0 0 1 4.4 4.4l-7.9 7.9a1.5 1.5 0 0 1-2.2-2.2l7.3-7.3"/>',
  search: '<circle cx="11" cy="11" r="6.5"/><path d="m20 20-4-4"/>',
  trash: '<path d="M4.5 7h15M10 11v6M14 11v6M6.5 7l.8 11.2a1.5 1.5 0 0 0 1.5 1.3h6.4a1.5 1.5 0 0 0 1.5-1.3L17.5 7M9.5 7V4.5h5V7"/>',
  mail: '<rect x="3.5" y="5" width="17" height="14" rx="1.5"/><path d="m4 6.5 8 6 8-6"/>',
  mailOpen: '<path d="M3.5 10v8.5A1.5 1.5 0 0 0 5 20h14a1.5 1.5 0 0 0 1.5-1.5V10L12 4.5 3.5 10Z"/><path d="m4 10.5 8 5.5 8-5.5"/>',
  checkAll: '<path d="m2.5 12.5 4.5 4.5 9.5-10M12.5 16l1 1 8-9"/>',
  download: '<path d="M12 4.5v11M7.5 11l4.5 4.5 4.5-4.5M5 19.5h14"/>',
  sun: '<circle cx="12" cy="12" r="3.75"/><path d="M12 3v1.5M12 19.5V21M5.6 5.6l1.1 1.1M17.3 17.3l1.1 1.1M3 12h1.5M19.5 12H21M5.6 18.4l1.1-1.1M17.3 6.7l1.1-1.1"/>',
  moon: '<path d="M19.5 14.5A7.5 7.5 0 0 1 9.5 4.5a7.5 7.5 0 1 0 10 10Z"/>',
  auto: '<circle cx="12" cy="12" r="8"/><path d="M12 4v16a8 8 0 0 0 0-16Z" fill="currentColor" stroke="none"/>',
  monitor: '<rect x="3.5" y="4.5" width="17" height="11.5" rx="1.5"/><path d="M8.5 20h7M12 16v4"/>',
  tablet: '<rect x="5.5" y="3" width="13" height="18" rx="1.5"/><path d="M11 17.5h2"/>',
  phone: '<rect x="7.5" y="3" width="9" height="18" rx="1.5"/><path d="M11 17.5h2"/>',
  external: '<path d="M14 4.5h5.5V10M19.5 4.5 11 13M17.5 13.5v5a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1v-11a1 1 0 0 1 1-1h5"/>',
  copy: '<rect x="8.5" y="8.5" width="11" height="11" rx="1.5"/><path d="M15.5 8.5V5.5a1 1 0 0 0-1-1h-9a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h3"/>',
  check: '<path d="m5 12.5 4.5 4.5L19 7.5"/>',
  bell: '<path d="M6.5 9a5.5 5.5 0 0 1 11 0c0 6 2.5 7.5 2.5 7.5H4s2.5-1.5 2.5-7.5M10 19.5a2 2 0 0 0 4 0"/>',
  bellOff: '<path d="m3.5 3.5 17 17M9 4.3A5.5 5.5 0 0 1 17.5 9c0 3 .6 4.9 1.3 6M16.5 16.5H4s2.5-1.5 2.5-7.5c0-.4 0-.8.1-1.2M10 19.5a2 2 0 0 0 4 0"/>',
  x: '<path d="m6 6 12 12M18 6 6 18"/>',
  back: '<path d="m14.5 18-6-6 6-6"/>',
  menu: '<path d="M4 7h16M4 12h16M4 17h16"/>',
  file: '<path d="M13.5 3.5H7A1.5 1.5 0 0 0 5.5 5v14A1.5 1.5 0 0 0 7 20.5h10a1.5 1.5 0 0 0 1.5-1.5V8.5l-5-5Z"/><path d="M13.5 3.5v5h5"/>',
  keyboard: '<rect x="2.5" y="6" width="19" height="12" rx="1.5"/><path d="M6.5 10h.01M10 10h.01M14 10h.01M17.5 10h.01M7.5 14h9"/>',
  link: '<path d="M10 14a3.8 3.8 0 0 0 5.4 0l3-3A3.8 3.8 0 0 0 13 5.6l-1 1M14 10a3.8 3.8 0 0 0-5.4 0l-3 3a3.8 3.8 0 0 0 5.4 5.4l1-1"/>',
  refresh: '<path d="M19.5 11A7.5 7.5 0 0 0 6 6.6L4.5 8M4.5 4v4h4M4.5 13A7.5 7.5 0 0 0 18 17.4l1.5-1.4M19.5 20v-4h-4"/>',
  chevronUp: '<path d="m6 15 6-6 6 6"/>',
  chevronDown: '<path d="m6 9 6 6 6-6"/>',
};

const icon = (name, cls = '') =>
  raw(`<svg class="icon ${cls}" viewBox="0 0 24 24" aria-hidden="true">${ICONS[name] || ''}</svg>`);

const prefs = {
  get(key, fallback) {
    try {
      const value = localStorage.getItem(`mailroom.${key}`);
      return value === null ? fallback : JSON.parse(value);
    } catch {
      return fallback;
    }
  },
  set(key, value) {
    try { localStorage.setItem(`mailroom.${key}`, JSON.stringify(value)); } catch { /* private mode */ }
  },
};

async function request(method, path, body) {
  const options = { method, headers: {} };
  if (body !== undefined) {
    options.headers['Content-Type'] = 'application/json';
    options.body = JSON.stringify(body);
  }
  const response = await fetch(`api/v1/${path}`, options);
  if (!response.ok) {
    let message = `${response.status} ${response.statusText}`;
    try { message = (await response.json()).error || message; } catch { /* not json */ }
    throw new Error(message);
  }
  if (response.status === 204) return null;
  const type = response.headers.get('content-type') || '';
  return type.includes('json') ? response.json() : response.text();
}

function debounce(fn, ms) {
  let timer;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  };
}

function formatSize(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB'];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit++; }
  return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}

function relativeTime(iso) {
  const date = new Date(iso);
  const seconds = (Date.now() - date.getTime()) / 1000;
  if (seconds < 45) return 'Now';
  if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
  const now = new Date();
  if (date.toDateString() === now.toDateString()) {
    return date.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
  }
  if (seconds < 6 * 86400) return date.toLocaleDateString(undefined, { weekday: 'short' });
  const sameYear = date.getFullYear() === now.getFullYear();
  return date.toLocaleDateString(undefined, sameYear ? { month: 'short', day: 'numeric' } : { year: 'numeric', month: 'short', day: 'numeric' });
}

const fullDate = (iso) =>
  new Date(iso).toLocaleString(undefined, { weekday: 'short', year: 'numeric', month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit', second: '2-digit' });

const displayName = (address) => (address ? address.name || address.address : 'Unknown sender');

function addressList(list) {
  return list.map((a, i) => html`${i ? html`<span class="sep">, </span>` : ''}${a.name ? html`${a.name} <span class="addr">&lt;${a.address}&gt;</span>` : a.address}`);
}

async function copy(text, label = 'Copied') {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const area = document.createElement('textarea');
    area.value = text;
    document.body.append(area);
    area.select();
    document.execCommand('copy');
    area.remove();
  }
  toast(label, 'success');
}

function toast(message, kind = 'info') {
  const element = document.createElement('div');
  element.className = `toast ${kind}`;
  element.innerHTML = toHtml(html`${kind === 'success' ? icon('check') : ''}<span>${message}</span>`);
  $('#toasts').append(element);
  setTimeout(() => {
    element.classList.add('leaving');
    setTimeout(() => element.remove(), 220);
  }, kind === 'error' ? 4500 : 1800);
}

/** A small promise-based replacement for `confirm()`. */
function confirmDialog({ title, message, confirm = 'Delete', danger = true }) {
  const modal = $('#modal');
  modal.innerHTML = toHtml(html`
    <form method="dialog">
      <div class="modal-body"><h3>${title}</h3><p>${message}</p></div>
      <div class="modal-actions">
        <button class="btn ghost" value="cancel">Cancel</button>
        <button class="btn ${danger ? 'danger' : 'primary'}" value="ok" autofocus>${confirm}</button>
      </div>
    </form>`);
  modal.showModal();
  return new Promise((resolve) => {
    modal.addEventListener('close', () => resolve(modal.returnValue === 'ok'), { once: true });
  });
}

function linkify(text) {
  const parts = [];
  const pattern = /\bhttps?:\/\/[^\s<>"']+[^\s<>"'.,;:!?)\]]/g;
  let last = 0;
  for (const match of text.matchAll(pattern)) {
    parts.push(escape(text.slice(last, match.index)));
    const url = match[0];
    parts.push(`<a href="${escape(url)}" target="_blank" rel="noopener noreferrer">${escape(url)}</a>`);
    last = match.index + url.length;
  }
  parts.push(escape(text.slice(last)));
  return raw(parts.join(''));
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

const state = {
  messages: [],
  total: 0,
  unread: 0,
  withAttachments: 0,
  matched: 0,
  query: '',
  loading: false,
  loaded: false,
  selectedId: null,
  detail: null,
  source: null,
  selection: new Set(),
  recipients: [],
  info: null,
  connected: false,
  fresh: new Map(), // id -> arrival time, for the highlight animation
  tab: prefs.get('tab', 'preview'),
  device: prefs.get('device', 'desktop'),
  blockRemote: prefs.get('blockRemote', false),
  theme: prefs.get('theme', 'system'),
  notify: prefs.get('notify', false) && 'Notification' in window && Notification.permission === 'granted',
  snippet: prefs.get('snippet', 'node'),
};

const appEl = $('#app');
const listEl = $('#list');
const detailEl = $('#detail');
const searchInput = $('#search-input');

const VIEWS = [
  { id: 'inbox', label: 'Inbox', icon: 'inbox', query: '', count: () => state.total },
  { id: 'unread', label: 'Unread', icon: 'unread', query: 'is:unread', count: () => state.unread, strong: true },
  { id: 'attachments', label: 'Attachments', icon: 'paperclip', query: 'has:attachment', count: () => state.withAttachments },
];

// ---------------------------------------------------------------------------
// Data loading
// ---------------------------------------------------------------------------

let listRequest = 0;

async function loadMessages({ append = false } = {}) {
  const requestId = ++listRequest;
  const params = new URLSearchParams({ limit: PAGE_SIZE, offset: append ? state.messages.length : 0 });
  if (state.query) params.set('query', state.query);
  state.loading = true;
  try {
    const data = await request('GET', `messages?${params}`);
    if (requestId !== listRequest) return;
    state.messages = append ? state.messages.concat(data.messages) : data.messages;
    state.total = data.total;
    state.unread = data.unread;
    state.matched = data.matched;
    state.loaded = true;
    const visible = new Set(state.messages.map((m) => m.id));
    for (const id of state.selection) if (!visible.has(id)) state.selection.delete(id);
  } catch (error) {
    toast(`Could not load messages: ${error.message}`, 'error');
  } finally {
    if (requestId === listRequest) state.loading = false;
  }
  renderList();
  renderNav();
  if (!state.selectedId) renderDetail();
}

async function loadRecipients() {
  try {
    state.recipients = await request('GET', 'recipients?limit=50');
  } catch {
    state.recipients = [];
  }
  renderRecipients();
}

const refreshRecipients = debounce(loadRecipients, 300);

async function loadStats() {
  try {
    const info = await request('GET', 'info');
    state.info = info;
    applyStats(info.stats);
  } catch { /* offline; SSE will retry */ }
  renderStatus();
}

function applyStats(stats) {
  state.total = stats.total;
  state.unread = stats.unread;
  state.withAttachments = stats.with_attachments;
  renderNav();
  renderCounts();
}

async function openMessage(id, { updateHash = true } = {}) {
  state.selectedId = id;
  state.source = null;
  if (updateHash && location.hash !== `#${id}`) history.replaceState(null, '', `#${id}`);
  appEl.classList.add('show-detail');
  renderListSelection();
  if (!state.detail || state.detail.id !== id) {
    detailEl.innerHTML = '<div class="skeleton" style="padding-top:72px"><div style="width:48%;height:16px"></div><div style="width:32%"></div><div style="width:38%"></div></div>';
  }
  try {
    const detail = await request('GET', `messages/${encodeURIComponent(id)}`);
    if (state.selectedId !== id) return;
    state.detail = detail;
    renderDetail();
    if (!detail.read) setRead([id], true);
  } catch (error) {
    if (state.selectedId !== id) return;
    state.detail = null;
    detailEl.innerHTML = toHtml(emptyState('Message unavailable', error.message));
  }
}

function closeMessage() {
  state.selectedId = null;
  state.detail = null;
  appEl.classList.remove('show-detail');
  if (location.hash) history.replaceState(null, '', location.pathname + location.search);
  renderListSelection();
  renderDetail();
}

function applyRead(ids, read) {
  const set = new Set(ids);
  for (const message of state.messages) if (set.has(message.id)) message.read = read;
  if (state.detail && set.has(state.detail.id)) state.detail.read = read;
}

async function setRead(ids, read) {
  applyRead(ids, read);
  renderList();
  renderDetailActions();
  try {
    if (ids.length === 1) await request('PATCH', `messages/${encodeURIComponent(ids[0])}`, { read });
    else await request('POST', 'messages/read', { ids, read });
  } catch (error) {
    toast(error.message, 'error');
  }
}

function neighbourOf(id) {
  const index = state.messages.findIndex((m) => m.id === id);
  if (index === -1) return null;
  return (state.messages[index + 1] || state.messages[index - 1] || null)?.id ?? null;
}

async function deleteMessages(ids) {
  if (!ids.length) return;
  const next = ids.includes(state.selectedId) ? neighbourOf(ids[ids.length - 1]) : null;
  removeMessages(ids);
  try {
    if (ids.length === 1) await request('DELETE', `messages/${encodeURIComponent(ids[0])}`);
    else await request('DELETE', 'messages', { ids });
    toast(ids.length === 1 ? 'Message deleted' : `${ids.length} messages deleted`, 'success');
  } catch (error) {
    toast(error.message, 'error');
    loadMessages();
  }
  if (next && !state.selectedId && window.innerWidth > 800) openMessage(next);
}

function removeMessages(ids) {
  const set = new Set(ids);
  const before = state.messages.length;
  state.messages = state.messages.filter((m) => !set.has(m.id));
  state.matched = Math.max(0, state.matched - (before - state.messages.length));
  for (const id of ids) state.selection.delete(id);
  if (state.selectedId && set.has(state.selectedId)) {
    state.selectedId = null;
    state.detail = null;
    appEl.classList.remove('show-detail');
    if (location.hash) history.replaceState(null, '', location.pathname + location.search);
  }
  renderList();
  if (!state.selectedId) renderDetail();
}

async function bulkDelete() {
  const ids = [...state.selection];
  if (ids.length) {
    if (await confirmDialog({ title: `Delete ${ids.length} message${ids.length > 1 ? 's' : ''}?`, message: 'This cannot be undone.' })) {
      deleteMessages(ids);
    }
    return;
  }
  if (!state.total) return;
  if (state.query) {
    if (!state.matched) return;
    const ok = await confirmDialog({
      title: `Delete ${state.matched} message${state.matched > 1 ? 's' : ''}?`,
      message: html`Every message matching <code>${state.query}</code> will be deleted. This cannot be undone.`,
    });
    if (!ok) return;
    await request('DELETE', `messages?query=${encodeURIComponent(state.query)}`);
    toast('Messages deleted', 'success');
    loadMessages();
    return;
  }
  const ok = await confirmDialog({
    title: 'Delete all messages?',
    message: `All ${state.total} message${state.total > 1 ? 's' : ''} will be deleted. This cannot be undone.`,
    confirm: 'Delete all',
  });
  if (!ok) return;
  await request('DELETE', 'messages');
  onCleared();
  toast('Mailbox cleared', 'success');
}

async function bulkMarkRead() {
  const ids = [...state.selection];
  if (ids.length) {
    const read = ids.some((id) => !state.messages.find((m) => m.id === id)?.read);
    await setRead(ids, read);
    return;
  }
  const body = state.query ? { query: state.query, read: true } : { read: true };
  applyRead(state.messages.map((m) => m.id), true);
  renderList();
  try {
    await request('POST', 'messages/read', body);
  } catch (error) {
    toast(error.message, 'error');
  }
}

// ---------------------------------------------------------------------------
// Live updates
// ---------------------------------------------------------------------------

const refreshFiltered = debounce(() => loadMessages(), 250);

function onCreated(summary) {
  if (state.query) {
    refreshFiltered();
  } else if (!state.messages.some((m) => m.id === summary.id)) {
    state.messages.unshift(summary);
    state.matched += 1;
    state.fresh.set(summary.id, performance.now());
    setTimeout(() => state.fresh.delete(summary.id), 2000);
    renderList();
    if (!state.selectedId) renderDetail();
  }
  refreshRecipients();
  if (state.notify && document.hidden) {
    try {
      const notification = new Notification(summary.subject || '(no subject)', {
        body: `From ${displayName(summary.from)}`,
        icon: 'assets/favicon.svg',
        tag: summary.id,
      });
      notification.onclick = () => { window.focus(); openMessage(summary.id); };
    } catch { /* notifications unavailable */ }
  }
}

function onCleared() {
  state.messages = [];
  state.selection.clear();
  state.total = 0;
  state.unread = 0;
  state.withAttachments = 0;
  state.matched = 0;
  state.recipients = [];
  closeMessage();
  renderList();
  renderNav();
  renderRecipients();
}

function connectEvents() {
  const source = new EventSource('api/v1/events');
  let disconnected = false;
  source.onopen = () => {
    setConnected(true);
    if (disconnected) {
      loadMessages();
      loadRecipients();
    }
    disconnected = false;
  };
  source.onerror = () => {
    disconnected = true;
    setConnected(false);
  };
  source.addEventListener('message.created', (e) => onCreated(JSON.parse(e.data)));
  source.addEventListener('message.updated', (e) => {
    const { ids, read } = JSON.parse(e.data);
    applyRead(ids, read);
    renderList();
    if (state.detail && ids.includes(state.detail.id)) renderDetailActions();
    refreshRecipients();
  });
  source.addEventListener('message.deleted', (e) => {
    removeMessages(JSON.parse(e.data).ids);
    refreshRecipients();
  });
  source.addEventListener('messages.cleared', () => onCleared());
  source.addEventListener('stats', (e) => applyStats(JSON.parse(e.data)));
  source.addEventListener('resync', () => { loadMessages(); loadRecipients(); });
}

function setConnected(connected) {
  state.connected = connected;
  renderStatus();
}

// ---------------------------------------------------------------------------
// Rendering: sidebar
// ---------------------------------------------------------------------------

function activeView() {
  return VIEWS.find((v) => v.query === state.query)?.id ?? null;
}

function renderNav() {
  const current = activeView();
  $('#nav').innerHTML = toHtml(VIEWS.map((view) => {
    const count = view.count();
    return html`
      <button class="nav-item ${view.id === current ? 'active' : ''}" data-view="${view.id}">
        ${icon(view.icon)}<span class="nav-label">${view.label}</span>
        ${count ? html`<span class="nav-count ${view.strong ? 'strong' : ''}">${count.toLocaleString()}</span>` : ''}
      </button>`;
  }));
  renderCounts();
}

function renderRecipients() {
  const container = $('#recipients');
  if (!state.recipients.length) {
    container.innerHTML = '<div class="nav-empty">Addresses that receive mail appear here.</div>';
    return;
  }
  container.innerHTML = toHtml(state.recipients.map((r) => {
    const active = state.query === `to:${r.address}`;
    return html`
      <button class="nav-item ${active ? 'active' : ''}" data-recipient="${r.address}" title="${r.address}">
        <span class="nav-label">${r.address}</span>
        <span class="nav-count ${r.unread ? 'strong' : ''}">${(r.unread || r.total).toLocaleString()}</span>
      </button>`;
  }));
}

function smtpEndpoint() {
  const smtp = state.info?.smtp_addr || '127.0.0.1:1025';
  const port = smtp.slice(smtp.lastIndexOf(':') + 1);
  let host = smtp.slice(0, smtp.lastIndexOf(':')).replace(/^\[|\]$/g, '');
  if (['0.0.0.0', '::', '127.0.0.1', '::1'].includes(host)) host = 'localhost';
  return { host, port };
}

function renderStatus() {
  const { host, port } = smtpEndpoint();
  const status = $('#status');
  status.classList.toggle('connected', state.connected);
  status.title = state.connected ? 'Receiving new messages in real time' : 'Lost connection to Mailroom; retrying';
  status.innerHTML = toHtml(html`
    <span class="status-dot"></span>
    <span><strong>${state.connected ? 'Listening' : 'Reconnecting…'}</strong>SMTP <code>${host}:${port}</code></span>`);
}

function setView(query) {
  searchInput.value = query;
  $('#search').classList.toggle('has-value', Boolean(query));
  state.query = query;
  appEl.classList.remove('nav-open');
  if (window.innerWidth <= 800 && state.selectedId) closeMessage();
  loadMessages();
  renderRecipients();
}

// ---------------------------------------------------------------------------
// Rendering: list
// ---------------------------------------------------------------------------

function renderCounts() {
  document.title = state.unread ? `(${state.unread}) Mailroom` : 'Mailroom';
  const toolbar = $('#list-toolbar');
  if (!toolbar) return;
  const selected = state.selection.size;
  const view = VIEWS.find((v) => v.query === state.query);
  const recipient = state.query.startsWith('to:') && !state.query.includes(' ') ? state.query.slice(3) : null;
  let label;
  if (selected) label = html`<strong>${selected}</strong> selected`;
  else if (!state.query || view) label = html`<strong>${state.matched.toLocaleString()}</strong> ${state.matched === 1 ? 'message' : 'messages'}`;
  else if (recipient) label = html`<strong>${state.matched.toLocaleString()}</strong> to <span class="filter">${recipient}</span>`;
  else label = html`<strong>${state.matched.toLocaleString()}</strong> of ${state.total.toLocaleString()} match`;

  const allChecked = state.messages.length > 0 && state.messages.every((m) => state.selection.has(m.id));
  const someChecked = selected > 0 && !allChecked;
  const scope = selected ? 'selected' : state.query ? 'matching' : 'all';
  toolbar.innerHTML = toHtml(html`
    <input type="checkbox" class="check" id="select-all" aria-label="Select all" ${allChecked ? 'checked' : ''} ${state.messages.length ? '' : 'disabled'}>
    <span class="count">${label}</span>
    <button class="icon-btn" data-action="refresh" title="Refresh (r)">${icon('refresh')}</button>
    <button class="icon-btn" data-action="bulk-read" title="Mark ${scope} as read" ${state.matched ? '' : 'disabled'}>${icon('checkAll')}</button>
    <button class="icon-btn danger" data-action="bulk-delete" title="Delete ${scope}" ${state.matched ? '' : 'disabled'}>${icon('trash')}</button>`);
  $('#select-all').indeterminate = someChecked;
  listEl.classList.toggle('selecting', selected > 0);
}

function rowHtml(message) {
  const classes = ['row'];
  if (!message.read) classes.push('unread');
  if (message.id === state.selectedId) classes.push('selected');
  // Re-renders continue the highlight where it was instead of restarting it.
  const freshSince = state.fresh.get(message.id);
  const freshStyle = freshSince === undefined ? '' : `animation-delay: -${Math.round(performance.now() - freshSince)}ms`;
  if (freshSince !== undefined) classes.push('fresh');
  const recipients = [...message.to, ...message.cc];
  const first = recipients[0] || message.bcc[0];
  const extra = (recipients.length || message.bcc.length) - 1;
  return html`
    <div class="${classes.join(' ')}" style="${freshStyle}" data-id="${message.id}" role="option" aria-selected="${message.id === state.selectedId}">
      <div class="row-gutter">
        ${message.read ? '' : raw('<span class="row-dot" aria-label="Unread"></span>')}
        <input type="checkbox" class="check" data-select="${message.id}" aria-label="Select message" ${state.selection.has(message.id) ? 'checked' : ''}>
      </div>
      <div class="row-main">
        <div class="row-top">
          <span class="row-from">${displayName(message.from)}</span>
          ${message.attachment_count ? html`<span class="row-icons" title="${message.attachment_count} attachment${message.attachment_count > 1 ? 's' : ''}">${icon('paperclip')}</span>` : ''}
          <time class="row-time" datetime="${message.received_at}" title="${fullDate(message.received_at)}">${relativeTime(message.received_at)}</time>
        </div>
        <div class="row-subject">${message.subject || '(no subject)'}</div>
        <div class="row-snippet">${first ? html`<span class="row-to">${first.address}${extra > 0 ? ` +${extra}` : ''}</span> — ` : ''}${message.snippet || 'No text content'}</div>
      </div>
    </div>`;
}

function renderList() {
  if (!state.loaded) {
    listEl.innerHTML = '<div class="skeleton"><div></div><div style="width:70%"></div><div style="width:85%"></div><div style="width:60%"></div></div>';
    return;
  }
  if (!state.messages.length) {
    listEl.innerHTML = toHtml(state.query
      ? html`<div class="list-empty"><strong>No results</strong><p>Nothing matches <code>${state.query}</code>.</p><button class="btn small" data-action="clear-search">Clear search</button></div>`
      : html`<div class="list-empty"><strong>No messages</strong><p>New mail appears here as soon as it arrives.</p></div>`);
    renderCounts();
    return;
  }
  const more = state.messages.length < state.matched
    ? html`<div class="list-more">${state.loading ? 'Loading…' : `Showing ${state.messages.length.toLocaleString()} of ${state.matched.toLocaleString()}`}</div>`
    : '';
  listEl.innerHTML = toHtml(html`${state.messages.map(rowHtml)}${more}`);
  renderCounts();
}

function renderListSelection() {
  for (const row of listEl.querySelectorAll('.row')) {
    const selected = row.dataset.id === state.selectedId;
    row.classList.toggle('selected', selected);
    row.setAttribute('aria-selected', selected);
  }
  listEl.querySelector('.row.selected')?.scrollIntoView({ block: 'nearest' });
}

function updateTimes() {
  for (const time of document.querySelectorAll('.row-time')) time.textContent = relativeTime(time.getAttribute('datetime'));
}

// ---------------------------------------------------------------------------
// Rendering: message
// ---------------------------------------------------------------------------

function emptyState(title, text, extra = '') {
  return html`<div class="empty"><div><strong>${title}</strong><div>${text}</div>${extra}</div></div>`;
}

function renderDetail() {
  if (state.selectedId && state.detail) {
    renderMessage();
    return;
  }
  if (state.selectedId) return; // still loading
  if (state.loaded && state.total === 0) {
    renderOnboarding();
    return;
  }
  detailEl.innerHTML = toHtml(emptyState('No message selected', 'Choose a message to inspect it.', html`
    <div class="keys">
      <span><kbd>J</kbd><kbd>K</kbd> Navigate</span>
      <span><kbd>/</kbd> Search</span>
      <span><kbd>?</kbd> Shortcuts</span>
    </div>`));
}

const TABS = [
  { id: 'preview', label: 'Preview', key: '1' },
  { id: 'text', label: 'Plain text', key: '2' },
  { id: 'links', label: 'Links', key: '3' },
  { id: 'attachments', label: 'Attachments', key: '4' },
  { id: 'headers', label: 'Headers', key: '5' },
  { id: 'source', label: 'Source', key: '6' },
  { id: 'delivery', label: 'Delivery', key: '7' },
];

function tabAvailable(tab, detail) {
  if (tab === 'preview') return Boolean(detail.html);
  if (tab === 'text') return detail.text !== null;
  if (tab === 'attachments') return detail.attachments.length + detail.inline.length > 0;
  return true;
}

function currentTab(detail) {
  if (tabAvailable(state.tab, detail)) return state.tab;
  return ['preview', 'text', 'source'].find((t) => tabAvailable(t, detail));
}

function tabCount(tab, detail) {
  if (tab === 'links') return detail.links.length || null;
  if (tab === 'attachments') return detail.attachments.length + detail.inline.length || null;
  if (tab === 'headers') return detail.headers.length;
  return null;
}

function renderMessage() {
  const d = state.detail;
  const tab = currentTab(d);
  const headerBcc = new Set(d.headers.filter((h) => h.name.toLowerCase() === 'bcc').flatMap((h) => h.value.toLowerCase().match(/[^\s<>,;"]+@[^\s<>,;"]+/g) || []));
  const envelopeOnly = d.bcc.some((a) => !headerBcc.has(a.address.toLowerCase()));
  const rows = [
    ['From', d.from ? addressList([d.from]) : html`<span class="addr">No From header</span>`],
    ['To', d.to.length ? addressList(d.to) : html`<span class="addr">No To header</span>`],
  ];
  if (d.cc.length) rows.push(['Cc', addressList(d.cc)]);
  if (d.bcc.length) rows.push(['Bcc', html`${addressList(d.bcc)}${envelopeOnly ? html`<span class="note" title="Delivered through the SMTP envelope without appearing in any header">envelope only</span>` : ''}`]);
  if (d.reply_to.length) rows.push(['Reply-To', addressList(d.reply_to)]);
  rows.push(['Date', html`<span title="Received ${fullDate(d.received_at)}">${fullDate(d.date || d.received_at)}</span>`]);
  if (d.codes.length) {
    rows.push([d.codes.length > 1 ? 'Codes' : 'Code', d.codes.map((code) => html`<span class="code-value">${code}<button class="link-copy" data-copy="${code}">Copy</button></span>`)]);
  }

  detailEl.innerHTML = toHtml(html`
    <div class="detail-toolbar">
      <button class="icon-btn back-btn" data-action="back" title="Back">${icon('back')}</button>
      <button class="icon-btn" data-action="prev" title="Previous message (k)">${icon('chevronUp')}</button>
      <button class="icon-btn" data-action="next" title="Next message (j)">${icon('chevronDown')}</button>
      <span class="spacer"></span>
      <span id="detail-actions" style="display:contents"></span>
    </div>
    <header class="msg-head">
      <h1 class="subject ${d.subject ? '' : 'none'}">${d.subject || '(no subject)'}</h1>
      <dl class="meta">${rows.map(([label, value]) => html`<dt>${label}</dt><dd>${value}</dd>`)}</dl>
    </header>
    <nav class="tabs" role="tablist">
      ${TABS.map((t) => {
        const count = tabCount(t.id, d);
        return html`<button class="tab ${t.id === tab ? 'active' : ''}" role="tab" data-tab="${t.id}" aria-selected="${t.id === tab}" ${tabAvailable(t.id, d) ? '' : raw('disabled title="Not in this message"')}>${t.label}${count ? html`<span class="count">${count}</span>` : ''}</button>`;
      })}
    </nav>
    <div class="tab-body" id="tab-body"></div>`);
  renderDetailActions();
  detailEl.querySelector('.tab.active')?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  renderTab(tab);
}

function renderDetailActions() {
  const d = state.detail;
  const actions = $('#detail-actions');
  if (!d || !actions) return;
  actions.innerHTML = toHtml(html`
    <button class="icon-btn" data-action="toggle-read" title="${d.read ? 'Mark as unread' : 'Mark as read'} (u)">${icon(d.read ? 'unread' : 'mailOpen')}</button>
    <a class="icon-btn" href="api/v1/messages/${d.id}/raw?download=1" title="Download .eml">${icon('download')}</a>
    <span class="divider"></span>
    <button class="icon-btn danger" data-action="delete" title="Delete (Del)">${icon('trash')}</button>`);
}

function selectTab(tab) {
  if (!state.detail || !tabAvailable(tab, state.detail)) return;
  state.tab = tab;
  prefs.set('tab', tab);
  for (const button of detailEl.querySelectorAll('.tab')) {
    const active = button.dataset.tab === tab;
    button.classList.toggle('active', active);
    button.setAttribute('aria-selected', active);
    if (active) button.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }
  renderTab(tab);
}

function renderTab(tab) {
  const body = $('#tab-body');
  if (!body) return;
  const d = state.detail;
  body.scrollTop = 0;
  switch (tab) {
    case 'preview': return renderPreview(body, d);
    case 'text':
      body.innerHTML = toHtml(html`<pre class="plain">${linkify(d.text)}</pre>`);
      return;
    case 'links': return renderLinks(body, d);
    case 'attachments': return renderAttachments(body, d);
    case 'headers': return renderHeaders(body, d);
    case 'source': return renderSource(body, d);
    case 'delivery': return renderDelivery(body, d);
    default:
      body.innerHTML = '';
  }
}

const DEVICES = [
  { id: 'desktop', label: 'Desktop', icon: 'monitor', width: null },
  { id: 'tablet', label: 'Tablet', icon: 'tablet', width: 768 },
  { id: 'mobile', label: 'Mobile', icon: 'phone', width: 375 },
];

function previewUrl(d) {
  return `api/v1/messages/${encodeURIComponent(d.id)}/html${state.blockRemote ? '?block_remote=1' : ''}`;
}

function renderPreview(body, d) {
  const device = DEVICES.find((x) => x.id === state.device) || DEVICES[0];
  body.innerHTML = toHtml(html`
    <div class="subbar">
      <div class="seg" role="group" aria-label="Preview width">
        ${DEVICES.map((x) => html`<button class="${x.id === device.id ? 'active' : ''}" data-device="${x.id}" title="${x.label}${x.width ? ` · ${x.width}px` : ''}">${icon(x.icon)}${x.label}</button>`)}
      </div>
      ${device.width ? html`<span class="width-label">${device.width}px</span>` : ''}
      <span class="spacer"></span>
      <label class="toggle" title="Block remote images, fonts and stylesheets, such as tracking pixels">
        <input type="checkbox" id="block-remote" ${state.blockRemote ? 'checked' : ''}><span class="track"></span>Block remote content
      </label>
      <a class="icon-btn" href="${previewUrl(d)}" target="_blank" rel="noopener" title="Open in new tab">${icon('external')}</a>
    </div>
    <div class="stage ${device.id}">
      <iframe class="frame" title="HTML preview" sandbox="allow-popups allow-popups-to-escape-sandbox allow-same-origin" referrerpolicy="no-referrer" src="${previewUrl(d)}"></iframe>
    </div>`);
}

function renderLinks(body, d) {
  if (!d.links.length) {
    body.innerHTML = toHtml(emptyState('No links', 'This message does not contain any links.'));
    return;
  }
  body.innerHTML = toHtml(html`
    <div class="pad">
      <ul class="items">
        ${d.links.map((link) => html`
          <li class="item">
            <div class="item-icon">${icon(link.url.startsWith('mailto:') ? 'mail' : 'link')}</div>
            <div class="item-text">
              <div class="item-title ${link.text ? '' : 'none'}">${link.text || 'Untitled link'}</div>
              <div class="item-sub url" title="${link.url}">${link.url}</div>
            </div>
            <div class="item-actions">
              <button class="icon-btn" data-copy="${link.url}" title="Copy URL">${icon('copy')}</button>
              <a class="icon-btn" href="${link.url}" target="_blank" rel="noopener noreferrer" title="Open">${icon('external')}</a>
            </div>
          </li>`)}
      </ul>
    </div>`);
}

function fileItem(part) {
  const isImage = part.content_type.startsWith('image/');
  const name = part.filename || (part.content_id ? `cid:${part.content_id}` : `part-${part.part_id}`);
  const ext = part.filename && part.filename.includes('.') ? part.filename.split('.').pop().slice(0, 4) : '';
  return html`
    <li class="item">
      <div class="item-icon">${isImage ? html`<img src="${part.url}" alt="" loading="lazy">` : ext || icon('file')}</div>
      <div class="item-text">
        <div class="item-title">${name}</div>
        <div class="item-sub">${formatSize(part.size)} · ${part.content_type}</div>
      </div>
      <div class="item-actions">
        <a class="icon-btn" href="${part.url}" target="_blank" rel="noopener" title="Open">${icon('external')}</a>
        <a class="icon-btn" href="${part.url}?download=1" title="Download">${icon('download')}</a>
      </div>
    </li>`;
}

function renderAttachments(body, d) {
  body.innerHTML = toHtml(html`
    <div class="pad">
      ${d.attachments.length ? html`<section class="section"><h3 class="section-title">Attachments<span class="count">${d.attachments.length}</span></h3><ul class="items">${d.attachments.map(fileItem)}</ul></section>` : ''}
      ${d.inline.length ? html`<section class="section"><h3 class="section-title">Inline images<span class="count">${d.inline.length}</span></h3><ul class="items">${d.inline.map(fileItem)}</ul></section>` : ''}
    </div>`);
}

function renderHeaders(body, d) {
  const rows = (filter) => d.headers
    .filter((h) => !filter || h.name.toLowerCase().includes(filter) || h.value.toLowerCase().includes(filter))
    .map((h) => html`<tr><th>${h.name}</th><td>${h.value}</td></tr>`);
  body.innerHTML = toHtml(html`
    <div class="pad">
      <input class="filter-input" id="header-filter" type="search" placeholder="Filter headers" autocomplete="off">
      <table class="table"><tbody id="header-rows">${rows('')}</tbody></table>
    </div>`);
  $('#header-filter').addEventListener('input', (e) => {
    $('#header-rows').innerHTML = toHtml(rows(e.target.value.trim().toLowerCase()));
  });
}

const SOURCE_LIMIT = 2 * 1024 * 1024;

async function renderSource(body, d) {
  if (!state.source || state.source.id !== d.id) {
    body.innerHTML = '<div class="skeleton"><div></div><div style="width:80%"></div><div style="width:65%"></div></div>';
    try {
      const text = await request('GET', `messages/${encodeURIComponent(d.id)}/raw`);
      state.source = { id: d.id, text };
    } catch (error) {
      body.innerHTML = toHtml(emptyState('Could not load source', error.message));
      return;
    }
    if (state.detail?.id !== d.id || currentTab(state.detail) !== 'source') return;
  }
  const full = state.source.text;
  const text = full.length > SOURCE_LIMIT ? full.slice(0, SOURCE_LIMIT) : full;
  const split = text.search(/\r?\n\r?\n/);
  const head = split === -1 ? text : text.slice(0, split);
  const rest = split === -1 ? '' : text.slice(split);
  const highlighted = head
    .split(/(\r?\n)/)
    .map((line) => {
      const match = /^([!-9;-~]+)(:)(.*)$/s.exec(line);
      return match
        ? `<span class="h-name">${escape(match[1])}</span><span class="h-sep">:</span><span class="h-value">${escape(match[3])}</span>`
        : `<span class="h-value">${escape(line)}</span>`;
    })
    .join('');
  body.innerHTML = toHtml(html`
    <div class="subbar">
      <span class="note">${formatSize(d.size)}${full.length > SOURCE_LIMIT ? ' · showing the first 2 MB' : ''}</span>
      <span class="spacer"></span>
      <button class="btn small" data-action="copy-source">${icon('copy')}Copy</button>
      <a class="btn small" href="api/v1/messages/${d.id}/raw?download=1">${icon('download')}Download .eml</a>
    </div>
    <pre class="plain">${raw(highlighted)}<span class="h-body">${rest}</span></pre>`);
}

function renderDelivery(body, d) {
  const e = d.envelope;
  const muted = (text) => html`<span class="muted">${text}</span>`;
  const missing = (header) => html`<span class="warn" title="Most providers add or require this header; mail without it is more likely to be flagged as spam.">Missing — add a ${header} header</span>`;
  const envelope = [
    ['MAIL FROM', e.mail_from || muted('<> (null sender)')],
    ['RCPT TO', e.rcpt_to.length ? e.rcpt_to.join(', ') : muted('none')],
  ];
  if (e.source === 'smtp') {
    envelope.push(['HELO / EHLO', e.helo || muted('—')]);
    envelope.push(['Client', e.remote_addr || muted('—')]);
    envelope.push(['Encryption', e.tls ? html`<span class="ok">STARTTLS</span>` : muted('None')]);
    envelope.push(['Authenticated as', e.auth_user || muted('Not authenticated')]);
  }
  const message = [
    ['Received', fullDate(d.received_at)],
    ['Received via', e.source === 'api' ? 'HTTP API' : 'SMTP'],
    ['Message-ID', d.message_id || missing('Message-ID')],
    ['Date header', d.date ? fullDate(d.date) : missing('Date')],
    ['Size', `${formatSize(d.size)} (${d.size.toLocaleString()} bytes)`],
    ['Mailroom ID', d.id],
  ];
  const table = (rows) => html`<table class="table"><tbody>${rows.map(([k, v]) => html`<tr><th>${k}</th><td>${v}</td></tr>`)}</tbody></table>`;
  body.innerHTML = toHtml(html`
    <div class="pad">
      <section class="section"><h3 class="section-title">SMTP envelope</h3>${table(envelope)}</section>
      <section class="section"><h3 class="section-title">Message</h3>${table(message)}</section>
    </div>`);
}

// ---------------------------------------------------------------------------
// Onboarding (empty mailbox)
// ---------------------------------------------------------------------------

function snippets() {
  const { host, port } = smtpEndpoint();
  const web = document.baseURI;
  const auth = state.info?.smtp_auth_required;
  return [
    { id: 'node', label: 'Node.js', code: `import nodemailer from "nodemailer";\n\nconst transport = nodemailer.createTransport({\n  host: "${host}",\n  port: ${port},\n  secure: false,${auth ? '\n  auth: { user: "USER", pass: "PASSWORD" },' : ''}\n});` },
    { id: 'laravel', label: 'Laravel', code: `MAIL_MAILER=smtp\nMAIL_HOST=${host}\nMAIL_PORT=${port}\nMAIL_USERNAME=${auth ? 'USER' : 'null'}\nMAIL_PASSWORD=${auth ? 'PASSWORD' : 'null'}\nMAIL_ENCRYPTION=null` },
    { id: 'django', label: 'Django', code: `EMAIL_BACKEND = "django.core.mail.backends.smtp.EmailBackend"\nEMAIL_HOST = "${host}"\nEMAIL_PORT = ${port}\nEMAIL_USE_TLS = False${auth ? '\nEMAIL_HOST_USER = "USER"\nEMAIL_HOST_PASSWORD = "PASSWORD"' : ''}` },
    { id: 'rails', label: 'Rails', code: `config.action_mailer.delivery_method = :smtp\nconfig.action_mailer.smtp_settings = {\n  address: "${host}",\n  port: ${port},\n}` },
    { id: 'python', label: 'Python', code: `import smtplib\nfrom email.message import EmailMessage\n\nmsg = EmailMessage()\nmsg["From"] = "app@example.com"\nmsg["To"] = "you@example.com"\nmsg["Subject"] = "Hello"\nmsg.set_content("It works.")\n\nwith smtplib.SMTP("${host}", ${port}) as smtp:\n    smtp.send_message(msg)` },
    { id: 'php', label: 'PHP', code: `; php.ini\nsendmail_path = "/usr/local/bin/mailroom sendmail -t -S ${host}:${port}"` },
    { id: 'curl', label: 'HTTP API', code: `# Send a message\ncurl -X POST ${web}api/v1/send \\\n  -H "Content-Type: application/json" \\\n  -d '{"from":"app@example.com","to":["you@example.com"],"subject":"Hello","text":"It works."}'\n\n# In tests: wait for a message and read its links and codes\ncurl "${web}api/v1/messages/wait?query=to:you@example.com&timeout=10"` },
  ];
}

function storageDescription() {
  const info = state.info;
  if (!info) return '—';
  const where = info.database ? info.database : 'In memory';
  return info.max_messages ? `${where} · newest ${info.max_messages.toLocaleString()} kept` : `${where} · unlimited`;
}

function renderOnboarding() {
  const { host, port } = smtpEndpoint();
  const list = snippets();
  const current = list.find((s) => s.id === state.snippet) || list[0];
  const info = state.info;
  const api = `${document.baseURI}api/v1`;
  const encryption = info?.smtp_tls ? (info.smtp_require_tls ? 'STARTTLS (required)' : 'STARTTLS (optional)') : 'None';
  detailEl.innerHTML = toHtml(html`
    <div class="onboard">
      <div class="onboard-inner">
        <div class="eyebrow"><span class="status-dot"></span>Ready to receive</div>
        <h1>Connect your app</h1>
        <p class="lead">Point your application's SMTP settings at Mailroom. Every message it sends is captured here and never reaches a real inbox.</p>
        <dl class="facts">
          <div class="fact"><dt>SMTP server</dt><dd><code>${host}:${port}</code></dd><button class="link-copy" data-copy="${host}:${port}">Copy</button></div>
          <div class="fact"><dt>Authentication</dt><dd>${info?.smtp_auth_required ? 'Required' : 'Not required — any credentials are accepted'}</dd><span></span></div>
          <div class="fact"><dt>Encryption</dt><dd>${encryption}</dd><span></span></div>
          <div class="fact"><dt>HTTP API</dt><dd><code>${api}</code></dd><button class="link-copy" data-copy="${api}">Copy</button></div>
          <div class="fact"><dt>Storage</dt><dd>${storageDescription()}</dd><span></span></div>
        </dl>
        <div class="snippet-tabs" role="tablist">
          ${list.map((s) => html`<button class="tab ${s.id === current.id ? 'active' : ''}" data-snippet="${s.id}">${s.label}</button>`)}
        </div>
        <div class="snippet">
          <pre><code>${current.code}</code></pre>
          <button class="icon-btn copy" data-copy="${current.code}" title="Copy">${icon('copy')}</button>
        </div>
        <div class="onboard-actions">
          <button class="btn primary" data-action="send-test">Send test message</button>
          <a href="${DOCS_URL}" target="_blank" rel="noopener">Testing with the API →</a>
        </div>
      </div>
    </div>`);
}

const TEST_EMAIL_HTML = `<!doctype html>
<html><head><meta name="viewport" content="width=device-width, initial-scale=1"></head>
<body style="margin:0;padding:0;background:#f4f4f5;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;color:#18181b">
  <table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="padding:40px 16px">
    <tr><td align="center">
      <table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="max-width:520px">
        <tr><td style="padding:0 4px 20px;font-size:15px;font-weight:600;letter-spacing:-0.01em">Mailroom</td></tr>
        <tr><td style="background:#ffffff;border:1px solid #e4e4e7;border-radius:10px;padding:36px 36px 32px">
          <h1 style="margin:0 0 12px;font-size:22px;line-height:1.3;font-weight:600;letter-spacing:-0.02em">Verify your email address</h1>
          <p style="margin:0 0 24px;font-size:15px;line-height:1.6;color:#52525b">This is a test message captured by Mailroom. Use the code below, or the button, to finish signing up.</p>
          <div style="margin:0 0 24px;padding:16px 0;border-radius:8px;background:#f4f4f5;text-align:center;font:600 26px ui-monospace,Menlo,monospace;letter-spacing:8px">482913</div>
          <a href="https://example.com/verify?token=mailroom-demo" style="display:inline-block;padding:11px 20px;border-radius:7px;background:#18181b;color:#ffffff;font-size:14px;font-weight:600;text-decoration:none">Verify email address</a>
          <p style="margin:28px 0 0;padding-top:20px;border-top:1px solid #e4e4e7;font-size:13px;line-height:1.6;color:#71717a">The code expires in 10 minutes. If you did not request this, you can ignore this message.</p>
        </td></tr>
        <tr><td style="padding:20px 4px 0;font-size:12px;line-height:1.6;color:#a1a1aa">Sent by Mailroom · Nothing was delivered to a real inbox.</td></tr>
      </table>
    </td></tr>
  </table>
</body></html>`;

async function sendTestEmail() {
  try {
    const summary = await request('POST', 'send', {
      from: 'Mailroom <hello@mailroom.test>',
      to: ['Alex Morgan <alex@example.com>'],
      subject: 'Verify your email address',
      text: 'Verify your email address\n\nThis is a test message captured by Mailroom.\n\nYour verification code is 482913.\n\nOr open: https://example.com/verify?token=mailroom-demo\n\nThe code expires in 10 minutes.\n',
      html: TEST_EMAIL_HTML,
      headers: { 'X-Mailroom-Test': 'true' },
      attachments: [{ filename: 'getting-started.txt', content_type: 'text/plain', content: btoa('Thanks for trying Mailroom.\n') }],
    });
    openMessage(summary.id);
  } catch (error) {
    toast(error.message, 'error');
  }
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

const OPERATORS = [
  ['from:', 'Sender'],
  ['to:', 'Any recipient, including Bcc'],
  ['subject:', 'Subject'],
  ['is:unread', 'Unread messages'],
  ['has:attachment', 'Has attachments'],
  ['after:2026-01-31', 'Received on or after'],
  ['before:2026-01-31', 'Received before'],
  ['"exact phrase"', 'Exact phrase'],
  ['-term', 'Exclude a term'],
];

function renderSearchHelp() {
  $('#search-help').innerHTML = toHtml(html`
    <h4>Search filters</h4>
    ${OPERATORS.map(([op, text]) => html`<button class="search-op" type="button" data-op="${op}"><code>${op}</code><span>${text}</span></button>`)}`);
}

const runSearch = debounce(() => {
  const query = searchInput.value.trim();
  if (query === state.query) return;
  state.query = query;
  loadMessages();
  renderRecipients();
}, 180);

function setSearch(value) {
  setView(value.trim());
}

// ---------------------------------------------------------------------------
// Theme, notifications, help
// ---------------------------------------------------------------------------

const THEMES = ['system', 'light', 'dark'];

function applyTheme() {
  if (state.theme === 'system') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = state.theme;
  const button = $('#theme-btn');
  button.innerHTML = toHtml(icon(state.theme === 'dark' ? 'moon' : state.theme === 'light' ? 'sun' : 'auto'));
  button.title = `Appearance: ${state.theme[0].toUpperCase()}${state.theme.slice(1)}`;
}

function renderNotifyButton() {
  const button = $('#notify-btn');
  button.innerHTML = toHtml(icon(state.notify ? 'bell' : 'bellOff'));
  button.classList.toggle('active', state.notify);
  button.title = state.notify ? 'Desktop notifications: on' : 'Desktop notifications: off';
}

async function toggleNotifications() {
  if (!('Notification' in window)) {
    toast('This browser does not support notifications', 'error');
    return;
  }
  if (state.notify) {
    state.notify = false;
  } else {
    const permission = Notification.permission === 'granted' ? 'granted' : await Notification.requestPermission();
    state.notify = permission === 'granted';
    if (!state.notify) toast('Notifications are blocked for this site', 'error');
  }
  prefs.set('notify', state.notify);
  renderNotifyButton();
  if (state.notify) toast('Notifications on for new mail while this tab is in the background', 'success');
}

function showHelp() {
  const keys = [
    ['Next / previous message', ['J', 'K']],
    ['Search', ['/']],
    ['Clear search or close', ['Esc']],
    ['Delete', ['Del']],
    ['Mark read / unread', ['U']],
    ['Switch tab', ['1', '–', '7']],
    ['Select message', ['X']],
    ['Refresh', ['R']],
    ['Show shortcuts', ['?']],
  ];
  const modal = $('#modal');
  modal.innerHTML = toHtml(html`
    <form method="dialog">
      <div class="modal-body">
        <h3>Keyboard shortcuts</h3>
        <div class="shortcuts">${keys.map(([label, combo]) => html`<span>${label}</span><span>${combo.map((k) => (k === '–' ? '–' : html`<kbd>${k}</kbd>`))}</span>`)}</div>
      </div>
      <div class="modal-actions"><button class="btn" value="close" autofocus>Done</button></div>
    </form>`);
  modal.showModal();
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

function moveSelection(step) {
  if (!state.messages.length) return;
  const index = state.messages.findIndex((m) => m.id === state.selectedId);
  const next = index === -1 ? (step > 0 ? 0 : state.messages.length - 1) : Math.min(state.messages.length - 1, Math.max(0, index + step));
  if (state.messages[next] && state.messages[next].id !== state.selectedId) openMessage(state.messages[next].id);
  if (next >= state.messages.length - 5 && state.messages.length < state.matched && !state.loading) loadMessages({ append: true });
}

function toggleSelect(id, checked) {
  if (checked) state.selection.add(id);
  else state.selection.delete(id);
  const box = listEl.querySelector(`[data-select="${CSS.escape(id)}"]`);
  if (box) box.checked = checked;
  renderCounts();
}

function bindEvents() {
  document.addEventListener('click', (event) => {
    const target = event.target;

    const copyButton = target.closest('[data-copy]');
    if (copyButton) {
      event.preventDefault();
      copy(copyButton.dataset.copy);
      return;
    }

    const checkbox = target.closest('[data-select]');
    if (checkbox) {
      event.stopPropagation();
      toggleSelect(checkbox.dataset.select, checkbox.checked);
      return;
    }

    if (target.id === 'select-all') {
      if (target.checked) state.messages.forEach((m) => state.selection.add(m.id));
      else state.selection.clear();
      renderList();
      return;
    }

    const row = target.closest('.row');
    if (row) {
      if (event.metaKey || event.ctrlKey || event.shiftKey) {
        toggleSelect(row.dataset.id, !state.selection.has(row.dataset.id));
        return;
      }
      openMessage(row.dataset.id);
      return;
    }

    const view = target.closest('[data-view]');
    if (view) return setView(VIEWS.find((v) => v.id === view.dataset.view).query);

    const recipient = target.closest('[data-recipient]');
    if (recipient) {
      const query = `to:${recipient.dataset.recipient}`;
      return setView(state.query === query ? '' : query);
    }

    const tab = target.closest('[data-tab]');
    if (tab) return selectTab(tab.dataset.tab);

    const device = target.closest('[data-device]');
    if (device) {
      state.device = device.dataset.device;
      prefs.set('device', state.device);
      renderTab('preview');
      return;
    }

    const snippet = target.closest('[data-snippet]');
    if (snippet) {
      state.snippet = snippet.dataset.snippet;
      prefs.set('snippet', state.snippet);
      renderOnboarding();
      return;
    }

    const op = target.closest('[data-op]');
    if (op) {
      event.preventDefault();
      const value = searchInput.value.trim();
      searchInput.value = value ? `${value} ${op.dataset.op}` : op.dataset.op;
      searchInput.focus();
      $('#search').classList.add('has-value');
      runSearch();
      return;
    }

    const action = target.closest('[data-action]')?.dataset.action;
    switch (action) {
      case 'back': return closeMessage();
      case 'prev': return moveSelection(-1);
      case 'next': return moveSelection(1);
      case 'open-nav': return appEl.classList.add('nav-open');
      case 'close-nav': return appEl.classList.remove('nav-open');
      case 'toggle-read': return state.detail && setRead([state.detail.id], !state.detail.read);
      case 'delete': return state.detail && deleteMessages([state.detail.id]);
      case 'bulk-delete': return bulkDelete();
      case 'bulk-read': return bulkMarkRead();
      case 'refresh': loadMessages(); return loadRecipients();
      case 'clear-search': return setSearch('');
      case 'send-test': return sendTestEmail();
      case 'copy-source': return state.source && copy(state.source.text, 'Source copied');
      default:
    }
  });

  document.addEventListener('change', (event) => {
    if (event.target.id === 'block-remote') {
      state.blockRemote = event.target.checked;
      prefs.set('blockRemote', state.blockRemote);
      renderTab('preview');
    }
  });

  searchInput.addEventListener('input', () => {
    $('#search').classList.toggle('has-value', Boolean(searchInput.value));
    $('#search-help').hidden = Boolean(searchInput.value);
    runSearch();
  });
  $('#search-help').addEventListener('mousedown', (event) => event.preventDefault());
  searchInput.addEventListener('focus', () => { $('#search-help').hidden = Boolean(searchInput.value); });
  searchInput.addEventListener('blur', () => setTimeout(() => { $('#search-help').hidden = true; }, 150));
  searchInput.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      if (searchInput.value) setSearch('');
      searchInput.blur();
    } else if (event.key === 'Enter') {
      $('#search-help').hidden = true;
      runSearch();
    } else if (event.key === 'ArrowDown') {
      event.preventDefault();
      searchInput.blur();
      moveSelection(1);
    }
  });

  listEl.addEventListener('scroll', () => {
    if (state.loading || state.messages.length >= state.matched) return;
    if (listEl.scrollTop + listEl.clientHeight > listEl.scrollHeight - 400) loadMessages({ append: true });
  });

  document.addEventListener('keydown', (event) => {
    if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) return;
    const typing = event.target.closest('input, textarea, select, [contenteditable]');
    if (typing || $('#modal').open) return;
    const key = event.key;
    if (key === '/') { event.preventDefault(); searchInput.focus(); searchInput.select(); return; }
    if (key === '?') { showHelp(); return; }
    if (key === 'j' || key === 'ArrowDown') { event.preventDefault(); moveSelection(1); return; }
    if (key === 'k' || key === 'ArrowUp') { event.preventDefault(); moveSelection(-1); return; }
    if (key === 'r') { loadMessages(); loadRecipients(); return; }
    if (key === 'Escape') {
      if (appEl.classList.contains('nav-open')) { appEl.classList.remove('nav-open'); return; }
      if (state.selection.size) { state.selection.clear(); renderList(); return; }
      if (state.selectedId) closeMessage();
      else if (state.query) setSearch('');
      return;
    }
    if (key === 'x' && state.selectedId) { toggleSelect(state.selectedId, !state.selection.has(state.selectedId)); return; }
    if ((key === 'Delete' || key === 'Backspace' || key === '#') && (state.selection.size || state.selectedId)) {
      event.preventDefault();
      if (state.selection.size) bulkDelete();
      else deleteMessages([state.selectedId]);
      return;
    }
    if (key === 'u' && state.detail) { setRead([state.detail.id], !state.detail.read); return; }
    const tab = TABS.find((t) => t.key === key);
    if (tab) selectTab(tab.id);
  });

  $('#theme-btn').addEventListener('click', () => {
    state.theme = THEMES[(THEMES.indexOf(state.theme) + 1) % THEMES.length];
    prefs.set('theme', state.theme);
    applyTheme();
  });
  $('#notify-btn').addEventListener('click', toggleNotifications);
  $('#help-btn').addEventListener('click', showHelp);

  window.addEventListener('hashchange', () => {
    const id = location.hash.slice(1);
    if (id && id !== state.selectedId) openMessage(id, { updateHash: false });
  });

  // Resizable list pane.
  const resizer = $('#resizer');
  const saved = prefs.get('listWidth', null);
  if (saved) document.documentElement.style.setProperty('--list-width', `${saved}px`);
  resizer.addEventListener('pointerdown', (event) => {
    event.preventDefault();
    resizer.classList.add('dragging');
    resizer.setPointerCapture(event.pointerId);
    const frames = document.querySelectorAll('iframe');
    frames.forEach((f) => { f.style.pointerEvents = 'none'; });
    const offset = $('.list-pane').getBoundingClientRect().left;
    const move = (e) => {
      const width = Math.min(Math.max(e.clientX - offset, 300), Math.min(640, window.innerWidth - offset - 380));
      document.documentElement.style.setProperty('--list-width', `${width}px`);
    };
    const up = () => {
      resizer.classList.remove('dragging');
      frames.forEach((f) => { f.style.pointerEvents = ''; });
      const width = parseInt(getComputedStyle(document.documentElement).getPropertyValue('--list-width'), 10);
      prefs.set('listWidth', width);
      resizer.removeEventListener('pointermove', move);
      resizer.removeEventListener('pointerup', up);
    };
    resizer.addEventListener('pointermove', move);
    resizer.addEventListener('pointerup', up);
  });
}

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------

async function boot() {
  applyTheme();
  renderNotifyButton();
  $('#help-btn').innerHTML = toHtml(icon('keyboard'));
  $('.menu-btn').innerHTML = toHtml(icon('menu'));
  $('.nav-close').innerHTML = toHtml(icon('x'));
  renderSearchHelp();
  renderNav();
  renderStatus();
  renderList();
  bindEvents();
  connectEvents();
  setInterval(updateTimes, 30_000);

  loadStats().then(() => {
    if (state.loaded && !state.total && !state.selectedId) renderOnboarding();
  });
  loadRecipients();
  await loadMessages();
  const id = location.hash.slice(1);
  if (id) openMessage(id, { updateHash: false });
}

boot();
