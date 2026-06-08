// Lattice Web Chat - Main Application
class LatticeApp {
  constructor() {
    this.apiUrl = '';
    this.wsUrl = '';
    this.username = '';
    this.storeFingerprint = '';
    this.ws = null;
    this.wsReconnectTimer = null;
    this.pollInterval = null;
    this.currentView = 'rooms';
    this.theme = localStorage.getItem('lattice-theme') || 'dark';

    // Module instances
    this.chat = null;
    this.contacts = null;
    this.rooms = null;
    this.settings = null;

    this.init();
  }

  init() {
    // Apply saved theme
    if (this.theme === 'light') {
      document.body.classList.add('light-theme');
    }

    // Load saved connection settings
    const savedApiUrl = localStorage.getItem('lattice-api-url');
    const savedUsername = localStorage.getItem('lattice-username');

    if (savedApiUrl) {
      document.getElementById('api-url').value = savedApiUrl;
    }
    if (savedUsername) {
      document.getElementById('username').value = savedUsername;
    }

    // Initialize modules
    this.chat = new ChatModule(this);
    this.contacts = new ContactsModule(this);
    this.rooms = new RoomsModule(this);
    this.settings = new SettingsModule(this);
    this.shortcuts = new KeyboardShortcuts(this);

    // Setup event listeners
    this.setupEventListeners();

    // Auto-connect if settings exist
    if (savedApiUrl && savedUsername) {
      setTimeout(() => this.connect(), 500);
    }
  }

  setupEventListeners() {
    // Connect button
    document.getElementById('connect-btn').addEventListener('click', () => this.connect());

    // Tab switching
    document.querySelectorAll('.tab').forEach(tab => {
      tab.addEventListener('click', (e) => {
        const tabName = e.target.dataset.tab;
        this.switchTab(tabName);
      });
    });

    // Theme toggle
    document.getElementById('theme-toggle').addEventListener('click', () => this.toggleTheme());

    // Sidebar toggle
    document.getElementById('toggle-sidebar').addEventListener('click', () => this.toggleSidebar());

    // Settings toggle
    document.getElementById('toggle-settings').addEventListener('click', () => this.toggleSettings());

    // Enter key in connection inputs
    document.getElementById('api-url').addEventListener('keydown', (e) => {
      if (e.key === 'Enter') this.connect();
    });
    document.getElementById('username').addEventListener('keydown', (e) => {
      if (e.key === 'Enter') this.connect();
    });

    // Notification permission
    if ('Notification' in window && Notification.permission === 'default') {
      Notification.requestPermission();
    }
  }

  async connect() {
    const apiUrlInput = document.getElementById('api-url').value.trim();
    const usernameInput = document.getElementById('username').value.trim();

    if (!apiUrlInput || !usernameInput) {
      this.showNotification('Please enter API URL and username', 'error');
      return;
    }

    this.apiUrl = apiUrlInput.replace(/\/$/, '');
    this.username = usernameInput;

    // Save settings
    localStorage.setItem('lattice-api-url', this.apiUrl);
    localStorage.setItem('lattice-username', this.username);

    try {
      // Check API health
      const health = await fetch(`${this.apiUrl}/health`).then(r => r.text());
      if (health !== 'ok') throw new Error('API not healthy');

      this.updateStatus('api', 'connected', 'API OK');

      // Get store identity
      const identity = await fetch(`${this.apiUrl}/identity`).then(r => r.json());
      this.storeFingerprint = identity.fingerprint;

      document.getElementById('user-display').textContent = this.username;
      document.getElementById('store-info').textContent =
        `${identity.display_name} (${identity.fingerprint.slice(0, 12)}...)`;

      // Enable chat input
      document.getElementById('message-input').disabled = false;
      document.getElementById('send-btn').disabled = false;

      // Connect WebSocket
      this.wsUrl = this.apiUrl.replace('http', 'ws');
      this.connectWebSocket();

      // Load initial data
      await this.rooms.loadRooms();
      await this.contacts.loadContacts();

      // Start polling as fallback
      if (this.pollInterval) clearInterval(this.pollInterval);
      this.pollInterval = setInterval(() => {
        if (this.chat.currentRoom) {
          this.chat.pollMessages();
        }
      }, 5000);

      this.showNotification('Connected successfully', 'success');

    } catch (e) {
      this.updateStatus('api', 'disconnected', 'Failed');
      this.showNotification(`Connection failed: ${e.message}`, 'error');
      console.error('Connect failed:', e);
    }
  }

  connectWebSocket() {
    if (this.ws) {
      this.ws.close();
    }

    if (this.wsReconnectTimer) {
      clearTimeout(this.wsReconnectTimer);
    }

    try {
      this.ws = new WebSocket(this.wsUrl);
      this.ws.binaryType = 'arraybuffer';

      this.ws.onopen = () => {
        this.updateStatus('ws', 'ws-connected', 'WS Live');
        console.log('WebSocket connected');
      };

      this.ws.onclose = () => {
        this.updateStatus('ws', 'disconnected', 'WS Off');
        // Auto-reconnect after 3s
        this.wsReconnectTimer = setTimeout(() => {
          if (this.apiUrl) this.connectWebSocket();
        }, 3000);
      };

      this.ws.onerror = () => {
        this.updateStatus('ws', 'disconnected', 'WS Err');
      };

      this.ws.onmessage = (event) => {
        // Binary protobuf frames from Store push
        console.log('WS message received, refreshing...');
        if (this.chat.currentRoom) {
          this.chat.loadMessages();
        }
        // Update room list to show new messages
        this.rooms.loadRooms();
      };

    } catch (e) {
      console.error('WebSocket connect failed:', e);
      this.updateStatus('ws', 'disconnected', 'WS Err');
    }
  }

  updateStatus(type, status, text) {
    const element = document.getElementById(`${type}-status`);
    if (element) {
      element.className = `status-badge ${status}`;
      element.textContent = text;
    }
  }

  switchTab(tabName) {
    this.currentView = tabName;

    // Update tab buttons
    document.querySelectorAll('.tab').forEach(tab => {
      if (tab.dataset.tab === tabName) {
        tab.classList.add('active');
      } else {
        tab.classList.remove('active');
      }
    });

    // Update tab panels
    document.querySelectorAll('.tab-panel').forEach(panel => {
      if (panel.id === `panel-${tabName}`) {
        panel.classList.add('active');
      } else {
        panel.classList.remove('active');
      }
    });

    // Load data for the active tab
    if (tabName === 'contacts') {
      this.contacts.loadContacts();
    } else if (tabName === 'rooms') {
      this.rooms.loadRooms();
    }
  }

  toggleTheme() {
    if (this.theme === 'dark') {
      this.theme = 'light';
      document.body.classList.add('light-theme');
    } else {
      this.theme = 'dark';
      document.body.classList.remove('light-theme');
    }
    localStorage.setItem('lattice-theme', this.theme);
  }

  toggleSidebar() {
    const sidebar = document.querySelector('.sidebar');
    sidebar.classList.toggle('collapsed');
  }

  toggleSettings() {
    const rightSidebar = document.querySelector('.right-sidebar');
    rightSidebar.classList.toggle('collapsed');
  }

  showNotification(message, type = 'info') {
    const notification = document.createElement('div');
    notification.className = `notification ${type}`;
    notification.textContent = message;
    document.body.appendChild(notification);

    setTimeout(() => {
      notification.style.animation = 'fadeOut 0.3s';
      setTimeout(() => notification.remove(), 300);
    }, 3000);

    // Browser notification for new messages
    if (type === 'message' && 'Notification' in window && Notification.permission === 'granted') {
      new Notification('Lattice', {
        body: message,
        icon: '/favicon.ico'
      });
    }
  }

  escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }

  formatTime(timestamp) {
    const date = new Date(timestamp);
    const now = new Date();
    const diff = now - date;

    // Less than 1 minute
    if (diff < 60000) {
      return 'Just now';
    }

    // Less than 1 hour
    if (diff < 3600000) {
      const minutes = Math.floor(diff / 60000);
      return `${minutes}m ago`;
    }

    // Today
    if (date.toDateString() === now.toDateString()) {
      return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    }

    // This week
    if (diff < 604800000) {
      const days = Math.floor(diff / 86400000);
      return `${days}d ago`;
    }

    // Older
    return date.toLocaleDateString();
  }

  getInitials(name) {
    if (!name) return '?';
    const parts = name.trim().split(/\s+/);
    if (parts.length === 1) {
      return parts[0].slice(0, 2).toUpperCase();
    }
    return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
  }

  getAvatarColor(id) {
    // Generate consistent color from ID
    let hash = 0;
    for (let i = 0; i < id.length; i++) {
      hash = id.charCodeAt(i) + ((hash << 5) - hash);
    }
    const hue = hash % 360;
    return `hsl(${hue}, 60%, 50%)`;
  }
}

// Initialize app when DOM is ready
let app;
document.addEventListener('DOMContentLoaded', () => {
  app = new LatticeApp();
});
