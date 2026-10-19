// Mailroom web UI. Plain ES modules, no framework and no build step: the
// whole UI ships inside the binary.

const PAGE_SIZE = 100;

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
  search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
  trash: '<path d="M4 7h16M10 11v6M14 11v6M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V4h6v3"/>',
  mail: '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="m3 7 9 6 9-6"/>',
  mailOpen: '<path d="M3 10v9a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-9l-9-6-9 6Z"/><path d="m3 10 9 6 9-6"/>',
  checkAll: '<path d="m2 12 5 5L18 6M13 16l1 1 8-9"/>',
  download: '<path d="M12 4v12M7 11l5 5 5-5M5 20h14"/>',
  paperclip: '<path d="m21 11-8.5 8.5a5 5 0 0 1-7-7L14 4a3.5 3.5 0 0 1 5 5l-8.5 8.5a2 2 0 0 1-3-3L15 7"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
  moon: '<path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5Z"/>',
  auto: '<circle cx="12" cy="12" r="9"/><path d="M12 3v18a9 9 0 0 0 0-18Z" fill="currentColor"/>',
  monitor: '<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>',
  tablet: '<rect x="5" y="2" width="14" height="20" rx="2"/><path d="M11 18h2"/>',
  phone: '<rect x="7" y="2" width="10" height="20" rx="2"/><path d="M11 18h2"/>',
  external: '<path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5"/>',
  copy: '<rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15V5a1 1 0 0 1 1-1h10"/>',
  check: '<path d="m5 12 5 5 9-10"/>',
  bell: '<path d="M6 8a6 6 0 1 1 12 0c0 7 3 8 3 8H3s3-1 3-8M10 20a2 2 0 0 0 4 0"/>',
  bellOff: '<path d="M3 3l18 18M8.7 3.6A6 6 0 0 1 18 8c0 3 .5 5 1.2 6.2M17 17H3s3-1 3-8c0-.6.1-1.2.2-1.7M10 20a2 2 0 0 0 4 0"/>',
  x: '<path d="M6 6l12 12M18 6 6 18"/>',
  back: '<path d="m15 18-6-6 6-6"/>',
  image: '<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-5-5L5 21"/>',
  file: '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8l-5-5Z"/><path d="M14 3v5h5"/>',
  lock: '<rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>',
  keyboard: '<rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M7 14h10"/>',
  link: '<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/>',
  inbox: '<path d="M3 13h5l1.5 3h5l1.5-3h5"/><path d="M5.5 5h13L21 13v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-5l2.5-8Z"/>',
  eye: '<path d="M2.5 12S6 5 12 5s9.5 7 9.5 7-3.5 7-9.5 7-9.5-7-9.5-7Z"/><circle cx="12" cy="12" r="3"/>',
  code: '<path d="m8 8-5 4 5 4M16 8l5 4-5 4M13.5 5l-3 14"/>',
  list: '<path d="M8 6h13M8 12h13M8 18h13M3.5 6h.01M3.5 12h.01M3.5 18h.01"/>',
  text: '<path d="M4 6h16M4 10h11M4 14h16M4 18h9"/>',
  send: '<path d="M21 3 10 14M21 3l-7 18-4-7-7-4 18-7Z"/>',
  key: '<circle cx="8" cy="15" r="4"/><path d="m11 12 9-9M17 6l3 3M14 9l2 2"/>',
  route: '<circle cx="6" cy="19" r="2.5"/><circle cx="18" cy="5" r="2.5"/><path d="M8.5 19H16a3.5 3.5 0 0 0 0-7H8a3.5 3.5 0 0 1 0-7h7.5"/>',
  refresh: '<path d="M20 11a8 8 0 0 0-14.7-4.4L4 8M4 4v4h4M4 13a8 8 0 0 0 14.7 4.4L20 16M20 20v-4h-4"/>',
  shield: '<path d="M12 3 5 6v6c0 4 3 7 7 9 4-2 7-5 7-9V6l-7-3Z"/>',
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
  if (seconds < 45) return 'now';
  if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
  if (seconds < 86400) return `${Math.round(seconds / 3600)}h`;
  const now = new Date();
  const sameYear = date.getFullYear() === now.getFullYear();
  if (seconds < 6 * 86400) return date.toLocaleDateString(undefined, { weekday: 'short' });
  return date.toLocaleDateString(undefined, sameYear ? { month: 'short', day: 'numeric' } : { year: 'numeric', month: 'short', day: 'numeric' });
}

const fullDate = (iso) =>
  new Date(iso).toLocaleString(undefined, { weekday: 'short', year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit' });

function hue(text) {
  let hash = 0;
  for (const char of text) hash = (hash * 31 + char.codePointAt(0)) | 0;
  return Math.abs(hash) % 360;
}

function initials(address) {
  if (!address) return '?';
  const source = address.name || address.address;
  const words = source.replace(/[@._-]+/g, ' ').trim().split(/\s+/).filter(Boolean);
  if (!words.length) return '?';
  const letters = address.name && words.length > 1 ? words[0][0] + words[words.length - 1][0] : words[0][0];
  return letters.toUpperCase();
}

function avatar(address) {
  const key = address ? address.address.toLowerCase() : '';
  return html`<div class="avatar" style="background: hsl(${hue(key)} 52% 52%)">${initials(address)}</div>`;
}

const displayName = (address) => (address ? address.name || address.address : 'Unknown sender');

function addressPills(list) {
  return list.map(
    (a) => html`<span class="addr-pill">${a.name ? html`${a.name} <span class="addr">&lt;${a.address}&gt;</span>` : a.address}</span>`,
  );
}

async function copy(text, label = 'Copied to clipboard') {
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
  }, kind === 'error' ? 4500 : 2200);
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
  matched: 0,
  query: '',
  loading: false,
  loaded: false,
  selectedId: null,
  detail: null,
  source: null,
  selection: new Set(),
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
  renderCounts();
  if (!state.selectedId) renderDetail();
}

async function openMessage(id, { updateHash = true } = {}) {
  state.selectedId = id;
  state.source = null;
  if (updateHash && location.hash !== `#${id}`) history.replaceState(null, '', `#${id}`);
  appEl.classList.add('show-detail');
  renderListSelection();
  if (!state.detail || state.detail.id !== id) {
    detailEl.innerHTML = '<div class="skeleton"><div style="width:55%;height:20px"></div><div style="width:35%"></div><div style="width:45%"></div></div>';
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
    detailEl.innerHTML = toHtml(placeholder('alert', 'Message unavailable', error.message));
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
  renderCounts();
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
      title: `Delete ${state.matched} matching message${state.matched > 1 ? 's' : ''}?`,
      message: html`Every message matching “${state.query}” will be removed.`,
    });
    if (!ok) return;
    await request('DELETE', `messages?query=${encodeURIComponent(state.query)}`);
    toast('Matching messages deleted', 'success');
    loadMessages();
    return;
  }
  const ok = await confirmDialog({
    title: 'Delete all messages?',
    message: `All ${state.total} message${state.total > 1 ? 's' : ''} in the mailbox will be removed.`,
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
  state.matched = 0;
  closeMessage();
  renderList();
  renderCounts();
}

function connectEvents() {
  const source = new EventSource('api/v1/events');
  let disconnected = false;
  source.onopen = () => {
    setConnected(true);
    if (disconnected) loadMessages();
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
  });
  source.addEventListener('message.deleted', (e) => removeMessages(JSON.parse(e.data).ids));
  source.addEventListener('messages.cleared', () => onCleared());
  source.addEventListener('stats', (e) => {
    const stats = JSON.parse(e.data);
    state.total = stats.total;
    state.unread = stats.unread;
    renderCounts();
  });
  source.addEventListener('resync', () => loadMessages());
}

function setConnected(connected) {
  state.connected = connected;
  const live = $('#live');
  live.classList.toggle('connected', connected);
  live.title = connected ? 'Receiving new messages in real time' : 'Disconnected — reconnecting…';
  $('.live-label', live).textContent = connected ? 'Live' : 'Reconnecting';
}

// ---------------------------------------------------------------------------
// Rendering: list
// ---------------------------------------------------------------------------

function renderCounts() {
  document.title = state.unread ? `(${state.unread}) Mailroom` : 'Mailroom';
  const selected = state.selection.size;
  const label = selected
    ? html`<strong>${selected}</strong> selected`
    : state.query
      ? html`<strong>${state.matched}</strong> of ${state.total} match`
      : html`<strong>${state.total}</strong> message${state.total === 1 ? '' : 's'}${state.unread ? html` · ${state.unread} unread` : ''}`;

  const allChecked = state.messages.length > 0 && state.messages.every((m) => state.selection.has(m.id));
  const someChecked = selected > 0 && !allChecked;
  const deleteTitle = selected ? 'Delete selected' : state.query ? 'Delete matching messages' : 'Delete all messages';
  const readTitle = selected ? 'Toggle read for selected' : state.query ? 'Mark matching as read' : 'Mark all as read';

  $('#list-toolbar').innerHTML = toHtml(html`
    <input type="checkbox" class="check" id="select-all" aria-label="Select all" ${allChecked ? 'checked' : ''} ${state.messages.length ? '' : 'disabled'}>
    <span class="count">${label}</span>
    <button class="icon-btn" data-action="refresh" title="Refresh">${icon('refresh')}</button>
    <button class="icon-btn" data-action="bulk-read" title="${readTitle}" ${state.total ? '' : 'disabled'}>${icon('checkAll')}</button>
    <button class="icon-btn danger" data-action="bulk-delete" title="${deleteTitle}" ${state.total ? '' : 'disabled'}>${icon('trash')}</button>`);
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
  const more = recipients.length + (recipients.length ? 0 : message.bcc.length) - 1;
  const to = first ? `${first.address}${more > 0 ? ` +${more}` : ''}` : '';
  return html`
    <div class="${classes.join(' ')}" style="${freshStyle}" data-id="${message.id}" role="option" aria-selected="${message.id === state.selectedId}">
      ${message.read ? '' : raw('<span class="unread-dot" aria-label="Unread"></span>')}
      <div class="row-lead">
        ${avatar(message.from)}
        <input type="checkbox" class="check" data-select="${message.id}" aria-label="Select message" ${state.selection.has(message.id) ? 'checked' : ''}>
      </div>
      <div class="row-body">
        <div class="row-line">
          <span class="row-from">${displayName(message.from)}</span>
          <time class="row-time" datetime="${message.received_at}" title="${fullDate(message.received_at)}">${relativeTime(message.received_at)}</time>
        </div>
        <div class="row-subject">${message.subject || '(no subject)'}</div>
        <div class="row-meta">
          ${to ? html`<span class="to" title="${recipients.map((a) => a.address).join(', ')}">→ ${to}</span>` : ''}
          ${message.attachment_count ? html`<span title="${message.attachment_count} attachment${message.attachment_count > 1 ? 's' : ''}">${icon('paperclip')}</span>` : ''}
          <span class="snippet">${message.snippet}</span>
        </div>
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
      ? html`<div class="list-empty">${icon('search')}<div>No messages match <strong>${state.query}</strong></div><p>Try fewer or different terms.</p><button class="btn small" data-action="clear-search">Clear search</button></div>`
      : html`<div class="list-empty">${icon('inbox')}<div>No messages yet</div><p>New mail appears here instantly.</p></div>`);
    renderCounts();
    return;
  }
  const more = state.messages.length < state.matched
    ? html`<div class="list-more" id="list-more">${state.loading ? 'Loading…' : `Showing ${state.messages.length} of ${state.matched}`}</div>`
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
  const current = listEl.querySelector('.row.selected');
  if (current) current.scrollIntoView({ block: 'nearest' });
}

function updateTimes() {
  for (const time of document.querySelectorAll('.row-time')) time.textContent = relativeTime(time.getAttribute('datetime'));
}

// ---------------------------------------------------------------------------
// Rendering: detail
// ---------------------------------------------------------------------------

function placeholder(iconName, title, text, extra = '') {
  return html`<div class="placeholder"><div><div class="big-icon">${icon(iconName in ICONS ? iconName : 'mail')}</div><h2>${title}</h2><p>${text}</p>${extra}</div></div>`;
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
  detailEl.innerHTML = toHtml(placeholder('mail', 'No message selected', 'Pick a message from the list to inspect it.', html`
    <div class="keys">
      <span><kbd>j</kbd><kbd>k</kbd> navigate</span>
      <span><kbd>/</kbd> search</span>
      <span><kbd>?</kbd> all shortcuts</span>
    </div>`));
}

const TABS = [
  { id: 'preview', label: 'HTML', icon: 'eye', key: '1' },
  { id: 'text', label: 'Text', icon: 'text', key: '2' },
  { id: 'links', label: 'Links', icon: 'link', key: '3' },
  { id: 'attachments', label: 'Attachments', icon: 'paperclip', key: '4' },
  { id: 'headers', label: 'Headers', icon: 'list', key: '5' },
  { id: 'source', label: 'Source', icon: 'code', key: '6' },
  { id: 'delivery', label: 'Delivery', icon: 'route', key: '7' },
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

function tabBadge(tab, detail) {
  if (tab === 'links' && detail.links.length) return detail.links.length;
  if (tab === 'attachments' && detail.attachments.length + detail.inline.length) return detail.attachments.length + detail.inline.length;
  if (tab === 'headers') return detail.headers.length;
  return null;
}

function renderMessage() {
  const d = state.detail;
  const tab = currentTab(d);
  const rows = [
    ['To', d.to],
    ['Cc', d.cc],
    ['Bcc', d.bcc],
    ['Reply-To', d.reply_to],
  ].filter(([, list]) => list.length);
  const headerBcc = new Set(d.headers.filter((h) => h.name.toLowerCase() === 'bcc').flatMap((h) => h.value.toLowerCase().match(/[^\s<>,;"]+@[^\s<>,;"]+/g) || []));
  const envelopeOnly = d.bcc.filter((a) => !headerBcc.has(a.address.toLowerCase()));

  detailEl.innerHTML = toHtml(html`
    <div class="detail-head">
      <div class="detail-top">
        <button class="icon-btn back-btn" data-action="back" title="Back to list">${icon('back')}</button>
        <h1 class="subject ${d.subject ? '' : 'none'}">${d.subject || '(no subject)'}</h1>
        <div class="detail-actions" id="detail-actions"></div>
      </div>
      <div class="sender">
        ${avatar(d.from)}
        <div class="sender-text">
          <div class="sender-name">${d.from ? html`${d.from.name || d.from.address}${d.from.name ? html`<span class="addr">&lt;${d.from.address}&gt;</span>` : ''}` : html`<span class="addr">No From header</span>`}</div>
        </div>
        <time class="sender-date" title="Received ${fullDate(d.received_at)}">${fullDate(d.date || d.received_at)}</time>
      </div>
      ${rows.length ? html`<dl class="recipients">${rows.map(([label, list]) => html`
        <dt>${label}</dt>
        <dd>${addressPills(list)}${label === 'Bcc' && envelopeOnly.length ? html`<span class="chip-note" title="These recipients were in the SMTP envelope but not in any header">envelope only</span>` : ''}</dd>`)}
      </dl>` : ''}
      ${d.codes.length ? html`
        <div class="codes">
          <span class="codes-label">${icon('key')} Verification code</span>
          ${d.codes.map((code) => html`<button class="code-chip" data-copy="${code}" title="Copy code">${code}${icon('copy')}</button>`)}
        </div>` : ''}
      <nav class="tabs" role="tablist">
        ${TABS.map((t) => {
          const badge = tabBadge(t.id, d);
          return html`<button class="tab ${t.id === tab ? 'active' : ''}" role="tab" data-tab="${t.id}" aria-selected="${t.id === tab}" ${tabAvailable(t.id, d) ? '' : raw('disabled title="Not in this message"')}>${t.label}${badge ? html`<span class="badge">${badge}</span>` : ''}</button>`;
        })}
      </nav>
    </div>
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
    <button class="icon-btn" data-action="toggle-read" title="${d.read ? 'Mark as unread' : 'Mark as read'} (u)">${icon(d.read ? 'mail' : 'mailOpen')}</button>
    <a class="icon-btn" href="api/v1/messages/${d.id}/raw?download=1" title="Download .eml">${icon('download')}</a>
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
      body.innerHTML = toHtml(html`<pre class="text-body">${linkify(d.text)}</pre>`);
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
    <div class="preview-toolbar">
      <div class="seg" role="group" aria-label="Preview width">
        ${DEVICES.map((x) => html`<button class="${x.id === device.id ? 'active' : ''}" data-device="${x.id}" title="${x.label}${x.width ? ` (${x.width}px)` : ''}">${icon(x.icon)}<span>${x.label}</span></button>`)}
      </div>
      ${device.width ? html`<span class="width-label">${device.width}px</span>` : ''}
      <span class="spacer"></span>
      <label class="toggle" title="Block remote images, fonts and stylesheets (tracking pixels)">
        <input type="checkbox" id="block-remote" ${state.blockRemote ? 'checked' : ''}><span class="track"></span>Block remote content
      </label>
      <a class="icon-btn" href="${previewUrl(d)}" target="_blank" rel="noopener" title="Open HTML in a new tab">${icon('external')}</a>
    </div>
    <div class="preview-stage ${device.id}">
      <iframe class="preview-frame" title="HTML preview" sandbox="allow-popups allow-popups-to-escape-sandbox allow-same-origin" referrerpolicy="no-referrer" src="${previewUrl(d)}"></iframe>
    </div>`);
}

function renderLinks(body, d) {
  if (!d.links.length) {
    body.innerHTML = toHtml(placeholder('link', 'No links', 'This message does not contain any links.'));
    return;
  }
  body.innerHTML = toHtml(html`
    <div class="tab-pad">
      <h3 class="section-title">${d.links.length} link${d.links.length > 1 ? 's' : ''}</h3>
      <ul class="link-list">
        ${d.links.map((link) => html`
          <li class="link-item">
            <span class="icon-wrap">${icon(link.url.startsWith('mailto:') ? 'mail' : 'link')}</span>
            <div class="link-text">
              <div class="link-label ${link.text ? '' : 'none'}">${link.text || 'No link text'}</div>
              <div class="link-url" title="${link.url}">${link.url}</div>
            </div>
            <div class="link-actions">
              <button class="icon-btn" data-copy="${link.url}" title="Copy URL">${icon('copy')}</button>
              <a class="icon-btn" href="${link.url}" target="_blank" rel="noopener noreferrer" title="Open link">${icon('external')}</a>
            </div>
          </li>`)}
      </ul>
    </div>`);
}

function fileCard(part) {
  const isImage = part.content_type.startsWith('image/');
  const name = part.filename || (part.content_id ? `cid:${part.content_id}` : `part-${part.part_id}`);
  const ext = (part.filename || '').split('.').pop();
  return html`
    <a class="file" href="${part.url}?download=1" title="Download ${name}">
      <div class="file-thumb">
        ${isImage ? html`<img src="${part.url}" alt="" loading="lazy">` : html`<div>${icon('file')}${ext && ext !== part.filename ? html`<div class="ext">${ext}</div>` : ''}</div>`}
      </div>
      <div class="file-info">
        <div>
          <div class="file-name">${name}</div>
          <div class="file-meta">${formatSize(part.size)} · ${part.content_type}</div>
        </div>
        ${icon('download')}
      </div>
    </a>`;
}

function renderAttachments(body, d) {
  body.innerHTML = toHtml(html`
    <div class="tab-pad">
      ${d.attachments.length ? html`<h3 class="section-title">Attachments</h3><div class="files">${d.attachments.map(fileCard)}</div>` : ''}
      ${d.inline.length ? html`<h3 class="section-title">Inline (referenced by the HTML)</h3><div class="files">${d.inline.map(fileCard)}</div>` : ''}
    </div>`);
}

function renderHeaders(body, d) {
  const rows = (filter) => d.headers
    .filter((h) => !filter || h.name.toLowerCase().includes(filter) || h.value.toLowerCase().includes(filter))
    .map((h) => html`<tr><th>${h.name}</th><td>${h.value}</td></tr>`);
  body.innerHTML = toHtml(html`
    <div class="tab-pad">
      <input class="filter-input" id="header-filter" type="search" placeholder="Filter headers" autocomplete="off">
      <table class="kv"><tbody id="header-rows">${rows('')}</tbody></table>
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
      body.innerHTML = toHtml(placeholder('code', 'Could not load source', error.message));
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
      return match ? `<span class="h-name">${escape(match[1])}</span><span class="h-sep">:</span>${escape(match[3])}` : escape(line);
    })
    .join('');
  body.innerHTML = toHtml(html`
    <div class="source-toolbar">
      <span class="note">${formatSize(d.size)}${full.length > SOURCE_LIMIT ? ' · showing the first 2 MB' : ''}</span>
      <button class="btn small" data-action="copy-source">${icon('copy')} Copy</button>
      <a class="btn small" href="api/v1/messages/${d.id}/raw?download=1">${icon('download')} Download .eml</a>
    </div>
    <pre class="source">${raw(highlighted)}${rest}</pre>`);
}

function renderDelivery(body, d) {
  const e = d.envelope;
  const yes = (on, label) => on
    ? html`<span class="badge-ok">${icon('lock')} ${label}</span>`
    : html`<span class="badge-no">${label}</span>`;
  const rows = [
    ['Received', fullDate(d.received_at)],
    ['Source', e.source === 'api' ? 'HTTP API (POST /api/v1/send)' : 'SMTP'],
    ['MAIL FROM', e.mail_from ? e.mail_from : html`<span class="muted">&lt;&gt; (null sender)</span>`],
    ['RCPT TO', e.rcpt_to.length ? e.rcpt_to.join(', ') : html`<span class="muted">none</span>`],
  ];
  if (e.source === 'smtp') {
    rows.push(['HELO / EHLO', e.helo || html`<span class="muted">—</span>`]);
    rows.push(['Client', e.remote_addr || html`<span class="muted">—</span>`]);
    rows.push(['TLS', yes(e.tls, e.tls ? 'STARTTLS' : 'Plaintext')]);
    rows.push(['Authenticated as', e.auth_user || html`<span class="muted">not authenticated</span>`]);
  }
  const missing = (header) => html`<span class="warn" title="Most providers add or require this header; messages without it are more likely to be flagged as spam.">${icon('shield')} No ${header} header</span>`;
  rows.push(['Message-ID', d.message_id || missing('Message-ID')]);
  rows.push(['Date header', d.date ? fullDate(d.date) : missing('Date')]);
  rows.push(['Size', `${formatSize(d.size)} (${d.size.toLocaleString()} bytes)`]);
  rows.push(['Mailroom ID', d.id]);
  body.innerHTML = toHtml(html`
    <div class="tab-pad">
      <h3 class="section-title">SMTP envelope</h3>
      <table class="kv"><tbody>${rows.map(([k, v]) => html`<tr><th>${k}</th><td>${v}</td></tr>`)}</tbody></table>
    </div>`);
}

// ---------------------------------------------------------------------------
// Onboarding (empty mailbox)
// ---------------------------------------------------------------------------

function connectionDetails() {
  const smtp = state.info?.smtp_addr || '127.0.0.1:1025';
  const port = smtp.slice(smtp.lastIndexOf(':') + 1);
  let host = smtp.slice(0, smtp.lastIndexOf(':')).replace(/^\[|\]$/g, '');
  if (host === '0.0.0.0' || host === '::' || host === '127.0.0.1' || host === '::1') host = 'localhost';
  const web = document.baseURI;
  return { host, port, web };
}

function snippets() {
  const { host, port, web } = connectionDetails();
  const auth = state.info?.smtp_auth_required;
  return [
    { id: 'node', label: 'Node.js', code: `import nodemailer from "nodemailer";\n\nconst transport = nodemailer.createTransport({\n  host: "${host}",\n  port: ${port},\n  secure: false,${auth ? '\n  auth: { user: "USER", pass: "PASSWORD" },' : ''}\n});\n\nawait transport.sendMail({\n  from: "app@example.com",\n  to: "you@example.com",\n  subject: "Hello from Mailroom",\n  text: "It works!",\n});` },
    { id: 'laravel', label: 'Laravel', code: `# .env\nMAIL_MAILER=smtp\nMAIL_HOST=${host}\nMAIL_PORT=${port}\nMAIL_USERNAME=${auth ? 'USER' : 'null'}\nMAIL_PASSWORD=${auth ? 'PASSWORD' : 'null'}\nMAIL_ENCRYPTION=null` },
    { id: 'django', label: 'Django', code: `# settings.py\nEMAIL_BACKEND = "django.core.mail.backends.smtp.EmailBackend"\nEMAIL_HOST = "${host}"\nEMAIL_PORT = ${port}\nEMAIL_USE_TLS = False${auth ? '\nEMAIL_HOST_USER = "USER"\nEMAIL_HOST_PASSWORD = "PASSWORD"' : ''}` },
    { id: 'rails', label: 'Rails', code: `# config/environments/development.rb\nconfig.action_mailer.delivery_method = :smtp\nconfig.action_mailer.smtp_settings = {\n  address: "${host}",\n  port: ${port},\n}` },
    { id: 'python', label: 'Python', code: `import smtplib\nfrom email.message import EmailMessage\n\nmsg = EmailMessage()\nmsg["From"] = "app@example.com"\nmsg["To"] = "you@example.com"\nmsg["Subject"] = "Hello from Mailroom"\nmsg.set_content("It works!")\n\nwith smtplib.SMTP("${host}", ${port}) as smtp:\n    smtp.send_message(msg)` },
    { id: 'php', label: 'PHP mail()', code: `; php.ini — capture mail() through Mailroom's sendmail mode\nsendmail_path = "/usr/local/bin/mailroom sendmail -t -S ${host}:${port}"` },
    { id: 'curl', label: 'HTTP API', code: `curl -X POST ${web}api/v1/send \\\n  -H "Content-Type: application/json" \\\n  -d '{"from":"app@example.com","to":["you@example.com"],"subject":"Hello","text":"It works!"}'\n\n# In end-to-end tests: wait for the email, then read its link\ncurl "${web}api/v1/messages/wait?query=to:you@example.com&timeout=10"` },
  ];
}

function storageHint() {
  const info = state.info;
  if (!info) return '';
  const where = info.database ? `Messages are saved to ${info.database}` : 'Messages are kept in memory until Mailroom stops';
  return info.max_messages ? `${where}; the newest ${info.max_messages.toLocaleString()} are kept.` : `${where}.`;
}

function renderOnboarding() {
  const { host, port } = connectionDetails();
  const list = snippets();
  const current = list.find((s) => s.id === state.snippet) || list[0];
  detailEl.innerHTML = toHtml(html`
    <div class="onboard">
      <div class="onboard-inner">
        <div class="onboard-hero">
          <div class="big-icon">${icon('inbox')}</div>
          <div>
            <h2>Your inbox is ready</h2>
            <p>Point your app's SMTP settings at Mailroom. Every email it sends is caught here instead of reaching real people.</p>
          </div>
        </div>
        <div class="onboard-cards">
          <div class="onboard-card"><div class="label">SMTP host</div><div class="value">${host}</div></div>
          <div class="onboard-card"><div class="label">SMTP port</div><div class="value">${port}</div></div>
          <div class="onboard-card"><div class="label">Authentication</div><div class="value">${state.info?.smtp_auth_required ? 'Required' : 'None (any accepted)'}</div></div>
        </div>
        <div class="snippets">
          <div class="snippet-tabs" role="tablist">
            ${list.map((s) => html`<button class="tab ${s.id === current.id ? 'active' : ''}" data-snippet="${s.id}">${s.label}</button>`)}
          </div>
          <div class="snippet-body">
            <pre><code>${current.code}</code></pre>
            <button class="icon-btn copy-btn" data-copy="${current.code}" title="Copy">${icon('copy')}</button>
          </div>
        </div>
        <div class="onboard-actions">
          <button class="btn primary" data-action="send-test">${icon('send')} Send a test email</button>
          <span class="hint">${storageHint()}</span>
        </div>
      </div>
    </div>`);
}

const TEST_EMAIL_HTML = `<!doctype html>
<html><head><meta name="viewport" content="width=device-width, initial-scale=1"></head>
<body style="margin:0;background:#f4f4f7;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;color:#1f1f29">
  <table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="padding:32px 12px">
    <tr><td align="center">
      <table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="max-width:520px;background:#ffffff;border-radius:14px;overflow:hidden;box-shadow:0 2px 12px rgba(0,0,0,.06)">
        <tr><td style="background:linear-gradient(135deg,#7c7cf0,#4b4bc8);padding:28px 32px;color:#fff">
          <div style="font-size:13px;opacity:.85;letter-spacing:.04em;text-transform:uppercase">Mailroom</div>
          <div style="font-size:24px;font-weight:700;margin-top:6px">It works! 🎉</div>
        </td></tr>
        <tr><td style="padding:28px 32px;font-size:15px;line-height:1.6">
          <p style="margin:0 0 16px">This test message was caught by Mailroom. Nothing was delivered to a real inbox.</p>
          <p style="margin:0 0 8px">Mailroom spots one-time codes automatically. Your verification code is:</p>
          <div style="font:700 28px ui-monospace,Menlo,monospace;letter-spacing:6px;background:#f1f1fb;border-radius:10px;padding:14px 0;text-align:center;color:#4040b0;margin:0 0 20px">482 913</div>
          <p style="margin:0 0 22px">Links are collected too, so tests can follow them:</p>
          <a href="https://example.com/verify?token=mailroom-demo" style="display:inline-block;background:#5b5bd6;color:#fff;text-decoration:none;font-weight:600;padding:12px 22px;border-radius:9px">Verify email address</a>
          <p style="margin:24px 0 0;font-size:13px;color:#6b6b78">Try the device toggles above to preview this email at tablet and mobile widths.</p>
        </td></tr>
      </table>
    </td></tr>
  </table>
</body></html>`;

async function sendTestEmail() {
  try {
    const summary = await request('POST', 'send', {
      from: 'Mailroom <hello@mailroom.test>',
      to: ['You <you@example.com>'],
      subject: 'Welcome to Mailroom 👋',
      text: 'This test message was caught by Mailroom.\n\nYour verification code is: 482 913\n\nVerify: https://example.com/verify?token=mailroom-demo\n',
      html: TEST_EMAIL_HTML,
      headers: { 'X-Mailroom-Test': 'true' },
      attachments: [{ filename: 'welcome.txt', content_type: 'text/plain', content: btoa('Thanks for trying Mailroom!\n') }],
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
  ['from:', 'sender address or name'],
  ['to:', 'any recipient, incl. Bcc'],
  ['subject:', 'words in the subject'],
  ['is:unread', 'unread messages'],
  ['is:read', 'read messages'],
  ['has:attachment', 'with attachments'],
  ['after:2026-01-31', 'received on or after'],
  ['before:2026-01-31', 'received before'],
  ['"exact phrase"', 'match a phrase'],
  ['-word', 'exclude a term'],
];

function renderSearchHelp() {
  $('#search-help').innerHTML = toHtml(html`
    <h4>Search operators</h4>
    <div class="search-help-grid">
      ${OPERATORS.map(([op, text]) => html`<button class="search-op" type="button" data-op="${op}"><code>${op}</code><span>${text}</span></button>`)}
    </div>`);
}

const runSearch = debounce(() => {
  const query = searchInput.value.trim();
  if (query === state.query) return;
  state.query = query;
  loadMessages();
}, 180);

function setSearch(value) {
  searchInput.value = value;
  $('#search').classList.toggle('has-value', Boolean(value));
  state.query = value.trim();
  loadMessages();
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
  button.title = `Theme: ${state.theme} (click to change)`;
}

function renderNotifyButton() {
  const button = $('#notify-btn');
  button.innerHTML = toHtml(icon(state.notify ? 'bell' : 'bellOff'));
  button.classList.toggle('active', state.notify);
  button.title = state.notify ? 'Desktop notifications on' : 'Desktop notifications off';
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
  if (state.notify) toast('You will be notified about new mail while this tab is in the background', 'success');
}

function showHelp() {
  const keys = [
    ['Next / previous message', ['j', 'k']],
    ['Search', ['/']],
    ['Clear search / close', ['Esc']],
    ['Delete message', ['Del']],
    ['Toggle read / unread', ['u']],
    ['Switch tab', ['1', '–', '7']],
    ['Select message', ['x']],
    ['Refresh', ['r']],
    ['Show shortcuts', ['?']],
  ];
  const modal = $('#modal');
  modal.innerHTML = toHtml(html`
    <form method="dialog">
      <div class="modal-body">
        <h3>Keyboard shortcuts</h3>
        <div class="shortcuts">${keys.map(([label, combo]) => html`<span>${label}</span><span>${combo.map((k) => (k === '–' ? html`<span>–</span>` : html`<kbd>${k}</kbd>`))}</span>`)}</div>
      </div>
      <div class="modal-actions"><button class="btn" value="close" autofocus>Close</button></div>
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
  document.addEventListener('click', async (event) => {
    const target = event.target;

    const copyButton = target.closest('[data-copy]');
    if (copyButton) {
      event.preventDefault();
      copy(copyButton.dataset.copy, copyButton.classList.contains('code-chip') ? `Copied code ${copyButton.dataset.copy}` : 'Copied to clipboard');
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
      const insert = op.dataset.op;
      searchInput.value = value ? `${value} ${insert}` : insert;
      searchInput.focus();
      $('#search').classList.add('has-value');
      runSearch();
      return;
    }

    const action = target.closest('[data-action]')?.dataset.action;
    switch (action) {
      case 'back': return closeMessage();
      case 'toggle-read': return state.detail && setRead([state.detail.id], !state.detail.read);
      case 'delete': return state.detail && deleteMessages([state.detail.id]);
      case 'bulk-delete': return bulkDelete();
      case 'bulk-read': return bulkMarkRead();
      case 'refresh': return loadMessages();
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
    if (key === 'r') { loadMessages(); return; }
    if (key === 'Escape') {
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
    const move = (e) => {
      const width = Math.min(Math.max(e.clientX, 280), Math.min(720, window.innerWidth - 360));
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
  renderSearchHelp();
  renderList();
  bindEvents();
  connectEvents();
  setInterval(updateTimes, 30_000);

  request('GET', 'info').then((info) => {
    state.info = info;
    if (state.loaded && !state.total && !state.selectedId) renderOnboarding();
  }).catch(() => {});

  await loadMessages();
  const id = location.hash.slice(1);
  if (id) openMessage(id, { updateHash: false });
}

boot();
