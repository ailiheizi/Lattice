// Lattice Web Chat - Keyboard Shortcuts
class KeyboardShortcuts {
  constructor(app) {
    this.app = app;
    this.shortcuts = new Map();
    this.enabled = true;

    this.registerDefaultShortcuts();
    this.setupEventListeners();
  }

  setupEventListeners() {
    document.addEventListener('keydown', (e) => {
      if (!this.enabled) return;

      // Don't trigger shortcuts when typing in input fields
      if (e.target.tagName === 'INPUT' ||
          e.target.tagName === 'TEXTAREA' ||
          e.target.isContentEditable) {
        // Allow some shortcuts even in input fields
        if (!this.isInputShortcut(e)) return;
      }

      const key = this.getKeyCombo(e);
      const handler = this.shortcuts.get(key);

      if (handler) {
        e.preventDefault();
        handler(e);
      }
    });
  }

  getKeyCombo(e) {
    const parts = [];
    if (e.ctrlKey || e.metaKey) parts.push('Ctrl');
    if (e.altKey) parts.push('Alt');
    if (e.shiftKey) parts.push('Shift');
    parts.push(e.key);
    return parts.join('+');
  }

  isInputShortcut(e) {
    // Shortcuts that work in input fields
    const inputShortcuts = ['Ctrl+k', 'Ctrl+/', 'Escape'];
    const key = this.getKeyCombo(e);
    return inputShortcuts.includes(key);
  }

  register(keyCombo, handler, description = '') {
    this.shortcuts.set(keyCombo, handler);
    console.log(`Registered shortcut: ${keyCombo} - ${description}`);
  }

  unregister(keyCombo) {
    this.shortcuts.delete(keyCombo);
  }

  enable() {
    this.enabled = true;
  }

  disable() {
    this.enabled = false;
  }

  registerDefaultShortcuts() {
    // Search
    this.register('Ctrl+k', () => {
      document.getElementById('search-input').focus();
    }, 'Focus search');

    this.register('Ctrl+/', () => {
      document.getElementById('search-input').focus();
    }, 'Focus search (alternative)');

    // Navigation
    this.register('Ctrl+1', () => {
      this.app.switchTab('rooms');
    }, 'Switch to Rooms tab');

    this.register('Ctrl+2', () => {
      this.app.switchTab('contacts');
    }, 'Switch to Contacts tab');

    // UI toggles
    this.register('Ctrl+b', () => {
      this.app.toggleSidebar();
    }, 'Toggle sidebar');

    this.register('Ctrl+.', () => {
      this.app.toggleSettings();
    }, 'Toggle settings');

    this.register('Ctrl+Shift+t', () => {
      this.app.toggleTheme();
    }, 'Toggle theme');

    // Message actions
    this.register('Escape', () => {
      // Clear search
      this.app.chat.clearSearch();
      // Blur active element
      document.activeElement.blur();
    }, 'Clear search / Unfocus');

    this.register('Ctrl+Shift+c', () => {
      const messageInput = document.getElementById('message-input');
      messageInput.value = '';
      messageInput.focus();
    }, 'Clear message input');

    // Room navigation
    this.register('Alt+ArrowUp', () => {
      this.selectPreviousRoom();
    }, 'Select previous room');

    this.register('Alt+ArrowDown', () => {
      this.selectNextRoom();
    }, 'Select next room');

    // Scroll
    this.register('Ctrl+Home', () => {
      const container = document.getElementById('messages-container');
      container.scrollTop = 0;
    }, 'Scroll to top');

    this.register('Ctrl+End', () => {
      const container = document.getElementById('messages-container');
      container.scrollTop = container.scrollHeight;
    }, 'Scroll to bottom');

    // Help
    this.register('Ctrl+Shift+/', () => {
      this.showHelp();
    }, 'Show keyboard shortcuts');

    this.register('F1', () => {
      this.showHelp();
    }, 'Show help');
  }

  selectPreviousRoom() {
    const rooms = Array.from(this.app.rooms.rooms.keys());
    if (rooms.length === 0) return;

    const currentIndex = rooms.indexOf(this.app.chat.currentRoom);
    const prevIndex = currentIndex > 0 ? currentIndex - 1 : rooms.length - 1;
    const prevRoomId = rooms[prevIndex];
    const prevRoom = this.app.rooms.rooms.get(prevRoomId);

    if (prevRoom) {
      this.app.rooms.selectRoom(prevRoomId, prevRoom.name);
    }
  }

  selectNextRoom() {
    const rooms = Array.from(this.app.rooms.rooms.keys());
    if (rooms.length === 0) return;

    const currentIndex = rooms.indexOf(this.app.chat.currentRoom);
    const nextIndex = currentIndex < rooms.length - 1 ? currentIndex + 1 : 0;
    const nextRoomId = rooms[nextIndex];
    const nextRoom = this.app.rooms.rooms.get(nextRoomId);

    if (nextRoom) {
      this.app.rooms.selectRoom(nextRoomId, nextRoom.name);
    }
  }

  showHelp() {
    const shortcuts = [
      { key: 'Ctrl+K or Ctrl+/', action: 'Focus search' },
      { key: 'Ctrl+1', action: 'Switch to Rooms' },
      { key: 'Ctrl+2', action: 'Switch to Contacts' },
      { key: 'Ctrl+B', action: 'Toggle sidebar' },
      { key: 'Ctrl+.', action: 'Toggle settings' },
      { key: 'Ctrl+Shift+T', action: 'Toggle theme' },
      { key: 'Alt+↑/↓', action: 'Navigate rooms' },
      { key: 'Ctrl+Home/End', action: 'Scroll to top/bottom' },
      { key: 'Escape', action: 'Clear search / Unfocus' },
      { key: 'Enter', action: 'Send message' },
      { key: 'Shift+Enter', action: 'New line' },
      { key: 'F1 or Ctrl+Shift+/', action: 'Show this help' }
    ];

    const html = `
      <div style="max-height:400px;overflow-y:auto">
        <h3 style="margin-bottom:16px;color:var(--text-primary)">Keyboard Shortcuts</h3>
        <table style="width:100%;border-collapse:collapse">
          <thead>
            <tr style="border-bottom:1px solid var(--border-color)">
              <th style="text-align:left;padding:8px;color:var(--text-secondary);font-size:12px">Key</th>
              <th style="text-align:left;padding:8px;color:var(--text-secondary);font-size:12px">Action</th>
            </tr>
          </thead>
          <tbody>
            ${shortcuts.map(s => `
              <tr style="border-bottom:1px solid var(--bg-tertiary)">
                <td style="padding:8px">
                  <code style="background:var(--bg-tertiary);padding:4px 8px;border-radius:4px;font-size:12px">${s.key}</code>
                </td>
                <td style="padding:8px;color:var(--text-primary);font-size:13px">${s.action}</td>
              </tr>
            `).join('')}
          </tbody>
        </table>
      </div>
    `;

    this.showModal('Keyboard Shortcuts', html);
  }

  showModal(title, content) {
    // Remove existing modal
    const existing = document.getElementById('shortcuts-modal');
    if (existing) existing.remove();

    const modal = document.createElement('div');
    modal.id = 'shortcuts-modal';
    modal.className = 'modal-overlay';
    modal.innerHTML = `
      <div class="modal">
        <div class="modal-header">
          <div class="modal-title">${title}</div>
          <button class="modal-close" onclick="document.getElementById('shortcuts-modal').remove()">×</button>
        </div>
        <div class="modal-body">${content}</div>
        <div class="modal-footer">
          <button class="btn btn-primary" onclick="document.getElementById('shortcuts-modal').remove()">Close</button>
        </div>
      </div>
    `;

    document.body.appendChild(modal);

    // Close on overlay click
    modal.addEventListener('click', (e) => {
      if (e.target === modal) {
        modal.remove();
      }
    });

    // Close on Escape
    const escapeHandler = (e) => {
      if (e.key === 'Escape') {
        modal.remove();
        document.removeEventListener('keydown', escapeHandler);
      }
    };
    document.addEventListener('keydown', escapeHandler);
  }
}
