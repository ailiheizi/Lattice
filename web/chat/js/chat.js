// NextIM Web Chat - Chat Module
class ChatModule {
  constructor(app) {
    this.app = app;
    this.currentRoom = null;
    this.messages = new Map(); // roomId -> messages array
    this.lastPoll = new Map(); // roomId -> last timestamp

    this.setupEventListeners();
  }

  setupEventListeners() {
    // Send message
    document.getElementById('send-btn').addEventListener('click', () => this.sendMessage());

    // Enter to send (Shift+Enter for new line)
    document.getElementById('message-input').addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        this.sendMessage();
      }
    });

    // Search messages
    document.getElementById('search-btn').addEventListener('click', () => this.searchMessages());
    document.getElementById('search-input').addEventListener('keydown', (e) => {
      if (e.key === 'Enter') this.searchMessages();
    });

    // Clear search
    document.getElementById('clear-search').addEventListener('click', () => this.clearSearch());
  }

  selectRoom(roomId, roomName) {
    this.currentRoom = roomId;

    // Update header
    document.getElementById('chat-title').textContent = roomName || roomId;

    // Get room info
    const room = this.app.rooms.rooms.get(roomId);
    if (room) {
      const subtitle = `${room.memberCount || '?'} members · ${room.encrypted ? 'Encrypted' : 'Unencrypted'}`;
      document.getElementById('chat-subtitle').textContent = subtitle;
    }

    // Clear search
    this.clearSearch();

    // Load messages
    this.loadMessages();

    // Focus input
    document.getElementById('message-input').focus();
  }

  async sendMessage() {
    if (!this.currentRoom || !this.app.apiUrl) return;

    const input = document.getElementById('message-input');
    const text = input.value.trim();

    if (!text) return;

    // Clear input immediately
    input.value = '';

    // Adjust textarea height
    input.style.height = 'auto';

    try {
      const resp = await fetch(`${this.app.apiUrl}/messages`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          room_id: this.currentRoom,
          sender_fingerprint: this.app.username,
          text: text
        })
      });

      if (!resp.ok) throw new Error('Send failed');

      const data = await resp.json();

      // Add message to local cache
      const messages = this.messages.get(this.currentRoom) || [];
      messages.push({
        msg_id: data.msg_id,
        sender_fingerprint: this.app.username,
        text: text,
        timestamp: Date.now(),
        sent: true
      });
      this.messages.set(this.currentRoom, messages);

      // Update UI
      this.renderMessages();

      // Update room list
      this.app.rooms.updateRoomPreview(this.currentRoom, text);

    } catch (e) {
      console.error('Send message failed:', e);
      this.app.showNotification('Failed to send message', 'error');
      // Restore message to input
      input.value = text;
    }
  }

  async loadMessages() {
    if (!this.currentRoom || !this.app.apiUrl) return;

    try {
      const resp = await fetch(`${this.app.apiUrl}/messages/${this.currentRoom}`);
      if (!resp.ok) throw new Error('Load failed');

      const msgs = await resp.json();

      // Process messages
      const processedMsgs = msgs.map(m => ({
        ...m,
        sent: m.sender_fingerprint === this.app.username
      }));

      this.messages.set(this.currentRoom, processedMsgs);

      // Update last poll timestamp
      if (msgs.length > 0) {
        this.lastPoll.set(this.currentRoom, msgs[msgs.length - 1].timestamp);
      }

      this.renderMessages();

    } catch (e) {
      console.error('Load messages failed:', e);
      this.renderEmptyState('Failed to load messages');
    }
  }

  async pollMessages() {
    if (!this.currentRoom || !this.app.apiUrl) return;

    const since = this.lastPoll.get(this.currentRoom) || 0;

    try {
      const resp = await fetch(`${this.app.apiUrl}/messages/${this.currentRoom}?since=${since + 1}`);
      if (!resp.ok) return;

      const newMsgs = await resp.json();

      if (newMsgs.length > 0) {
        const messages = this.messages.get(this.currentRoom) || [];

        // Add new messages (avoid duplicates)
        for (const m of newMsgs) {
          if (!messages.find(existing => existing.msg_id === m.msg_id)) {
            messages.push({
              ...m,
              sent: m.sender_fingerprint === this.app.username
            });

            // Show notification for received messages
            if (!m.sent) {
              this.app.showNotification(`New message from ${m.sender_fingerprint}`, 'message');
            }
          }
        }

        this.messages.set(this.currentRoom, messages);
        this.lastPoll.set(this.currentRoom, newMsgs[newMsgs.length - 1].timestamp);

        this.renderMessages();
        this.app.rooms.updateRoomPreview(this.currentRoom, newMsgs[newMsgs.length - 1].text);
      }

    } catch (e) {
      // Silent fail for polling
    }
  }

  renderMessages() {
    const container = document.getElementById('messages-container');
    const messages = this.messages.get(this.currentRoom);

    if (!messages || messages.length === 0) {
      this.renderEmptyState('No messages yet. Start the conversation!');
      return;
    }

    // Store scroll position
    const wasAtBottom = container.scrollHeight - container.scrollTop <= container.clientHeight + 100;

    container.innerHTML = messages.map(m => this.renderMessage(m)).join('');

    // Scroll to bottom if was at bottom or new message
    if (wasAtBottom) {
      container.scrollTop = container.scrollHeight;
    }
  }

  renderMessage(msg) {
    const time = this.app.formatTime(msg.timestamp);
    const cls = msg.sent ? 'sent' : 'received';
    const senderName = msg.sender_fingerprint === this.app.username
      ? 'You'
      : msg.sender_fingerprint;

    let html = `<div class="message ${cls}">`;

    // Show sender name for received messages
    if (!msg.sent) {
      html += `<div class="message-sender">${this.app.escapeHtml(senderName)}</div>`;
    }

    html += `<div class="message-text">${this.app.escapeHtml(msg.text)}</div>`;
    html += `<div class="message-meta">`;
    html += `<span>${time}</span>`;

    // Show delivery status for sent messages
    if (msg.sent) {
      html += `<span>✓</span>`;
    }

    html += `</div></div>`;

    return html;
  }

  renderEmptyState(message) {
    const container = document.getElementById('messages-container');
    container.innerHTML = `
      <div class="empty-state">
        <div class="empty-state-icon">💬</div>
        <div>${message}</div>
      </div>
    `;
  }

  async searchMessages() {
    const query = document.getElementById('search-input').value.trim();

    if (!query || !this.app.apiUrl) return;

    try {
      const url = this.currentRoom
        ? `${this.app.apiUrl}/search?q=${encodeURIComponent(query)}&room_id=${this.currentRoom}&limit=20`
        : `${this.app.apiUrl}/search?q=${encodeURIComponent(query)}&limit=20`;

      const results = await fetch(url).then(r => r.json());

      const container = document.getElementById('search-results');

      if (results.length === 0) {
        container.innerHTML = '<div style="padding:12px;color:var(--text-muted)">No results found</div>';
      } else {
        container.innerHTML = results.map(r => `
          <div class="result-item">
            <div style="color:var(--accent-orange);margin-bottom:4px">${this.app.escapeHtml(r.snippet)}</div>
            <div style="color:var(--text-muted);font-size:11px">
              ${new Date(r.timestamp).toLocaleString()} · Score: ${r.score.toFixed(2)}
            </div>
          </div>
        `).join('');
      }

      container.style.display = 'block';

    } catch (e) {
      console.error('Search failed:', e);
      this.app.showNotification('Search failed', 'error');
    }
  }

  clearSearch() {
    document.getElementById('search-input').value = '';
    document.getElementById('search-results').style.display = 'none';
  }
}
