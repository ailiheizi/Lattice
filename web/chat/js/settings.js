// Lattice Web Chat - Settings Module
class SettingsModule {
  constructor(app) {
    this.app = app;
    this.settings = {
      notifications: true,
      soundEnabled: true,
      enterToSend: true,
      showTimestamps: true,
      compactMode: false
    };

    this.loadSettings();
    this.setupEventListeners();
  }

  loadSettings() {
    const saved = localStorage.getItem('lattice-settings');
    if (saved) {
      try {
        this.settings = { ...this.settings, ...JSON.parse(saved) };
      } catch (e) {
        console.error('Failed to load settings:', e);
      }
    }
    this.applySettings();
  }

  saveSettings() {
    localStorage.setItem('lattice-settings', JSON.stringify(this.settings));
    this.applySettings();
  }

  setupEventListeners() {
    // Notification toggle
    document.getElementById('toggle-notifications').addEventListener('click', () => {
      this.settings.notifications = !this.settings.notifications;
      this.updateToggle('toggle-notifications', this.settings.notifications);
      this.saveSettings();

      // Request permission if enabling
      if (this.settings.notifications && 'Notification' in window) {
        Notification.requestPermission();
      }
    });

    // Sound toggle
    document.getElementById('toggle-sound').addEventListener('click', () => {
      this.settings.soundEnabled = !this.settings.soundEnabled;
      this.updateToggle('toggle-sound', this.settings.soundEnabled);
      this.saveSettings();
    });

    // Enter to send toggle
    document.getElementById('toggle-enter-send').addEventListener('click', () => {
      this.settings.enterToSend = !this.settings.enterToSend;
      this.updateToggle('toggle-enter-send', this.settings.enterToSend);
      this.saveSettings();
    });

    // Timestamps toggle
    document.getElementById('toggle-timestamps').addEventListener('click', () => {
      this.settings.showTimestamps = !this.settings.showTimestamps;
      this.updateToggle('toggle-timestamps', this.settings.showTimestamps);
      this.saveSettings();
    });

    // Compact mode toggle
    document.getElementById('toggle-compact').addEventListener('click', () => {
      this.settings.compactMode = !this.settings.compactMode;
      this.updateToggle('toggle-compact', this.settings.compactMode);
      this.saveSettings();
    });

    // Export data
    document.getElementById('export-data-btn').addEventListener('click', () => this.exportData());

    // Clear cache
    document.getElementById('clear-cache-btn').addEventListener('click', () => this.clearCache());
  }

  applySettings() {
    // Update toggle UI
    this.updateToggle('toggle-notifications', this.settings.notifications);
    this.updateToggle('toggle-sound', this.settings.soundEnabled);
    this.updateToggle('toggle-enter-send', this.settings.enterToSend);
    this.updateToggle('toggle-timestamps', this.settings.showTimestamps);
    this.updateToggle('toggle-compact', this.settings.compactMode);

    // Apply compact mode
    if (this.settings.compactMode) {
      document.body.classList.add('compact-mode');
    } else {
      document.body.classList.remove('compact-mode');
    }
  }

  updateToggle(id, active) {
    const element = document.getElementById(id);
    if (element) {
      if (active) {
        element.classList.add('active');
      } else {
        element.classList.remove('active');
      }
    }
  }

  exportData() {
    try {
      const data = {
        version: '1.0',
        exported: new Date().toISOString(),
        settings: this.settings,
        contacts: this.app.contacts.contacts,
        rooms: Array.from(this.app.rooms.rooms.values()),
        messages: Array.from(this.app.chat.messages.entries()).map(([roomId, msgs]) => ({
          roomId,
          messages: msgs
        }))
      };

      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `lattice-export-${Date.now()}.json`;
      a.click();
      URL.revokeObjectURL(url);

      this.app.showNotification('Data exported successfully', 'success');

    } catch (e) {
      console.error('Export failed:', e);
      this.app.showNotification('Failed to export data', 'error');
    }
  }

  clearCache() {
    if (!confirm('Are you sure you want to clear all cached data? This will not delete data from the server.')) {
      return;
    }

    try {
      // Clear messages cache
      this.app.chat.messages.clear();
      this.app.chat.lastPoll.clear();

      // Clear rooms cache
      this.app.rooms.rooms.clear();

      // Clear contacts cache
      this.app.contacts.contacts = [];

      // Reload data
      if (this.app.apiUrl) {
        this.app.rooms.loadRooms();
        this.app.contacts.loadContacts();
      }

      this.app.showNotification('Cache cleared', 'success');

    } catch (e) {
      console.error('Clear cache failed:', e);
      this.app.showNotification('Failed to clear cache', 'error');
    }
  }

  showAbout() {
    alert(`Lattice Web Chat v1.0\n\nA decentralized instant messaging system.\n\nBuilt with modern web technologies.`);
  }
}
