// Lattice Web Chat - Contacts Module
class ContactsModule {
  constructor(app) {
    this.app = app;
    this.contacts = [];

    this.setupEventListeners();
  }

  setupEventListeners() {
    // Add contact button
    document.getElementById('add-contact-btn').addEventListener('click', () => this.addContact());

    // Enter key in contact inputs
    const inputs = ['contact-fingerprint', 'contact-name', 'contact-address'];
    inputs.forEach(id => {
      document.getElementById(id).addEventListener('keydown', (e) => {
        if (e.key === 'Enter') this.addContact();
      });
    });
  }

  async loadContacts() {
    if (!this.app.apiUrl) return;

    try {
      const resp = await fetch(`${this.app.apiUrl}/contacts`);
      if (!resp.ok) throw new Error('Load failed');

      this.contacts = await resp.json();
      this.renderContacts();

    } catch (e) {
      console.error('Load contacts failed:', e);
      this.renderEmptyState('Failed to load contacts');
    }
  }

  renderContacts() {
    const container = document.getElementById('contact-list');

    if (this.contacts.length === 0) {
      this.renderEmptyState('No contacts yet. Add your first contact below!');
      return;
    }

    container.innerHTML = this.contacts.map(contact => this.renderContact(contact)).join('');
  }

  renderContact(contact) {
    const displayName = contact.display_name || contact.fingerprint.slice(0, 12);
    const trustLevel = contact.trust_level || 'public';
    const trustClass = this.getTrustClass(trustLevel);
    const initials = this.app.getInitials(displayName);
    const avatarColor = this.app.getAvatarColor(contact.fingerprint);

    return `
      <div class="contact-item" data-fingerprint="${contact.fingerprint}">
        <div class="item-avatar" style="background: ${avatarColor}">
          ${initials}
          <div class="online-status"></div>
        </div>
        <div class="item-content">
          <div class="item-header">
            <div class="item-name">
              ${this.app.escapeHtml(displayName)}
              <span class="trust-badge ${trustClass}">${trustClass}</span>
            </div>
          </div>
          <div class="item-preview">
            ${this.app.escapeHtml(contact.store_address || 'No address')}
          </div>
        </div>
      </div>
    `;
  }

  renderEmptyState(message) {
    const container = document.getElementById('contact-list');
    container.innerHTML = `
      <div class="empty-list">
        <div style="font-size:32px;margin-bottom:8px">👥</div>
        <div>${message}</div>
      </div>
    `;
  }

  getTrustClass(trustLevel) {
    if (typeof trustLevel === 'number') {
      if (trustLevel >= 3) return 'verified';
      if (trustLevel >= 1) return 'tofu';
      return 'public';
    }
    return trustLevel || 'public';
  }

  async addContact() {
    const fingerprint = document.getElementById('contact-fingerprint').value.trim();
    const name = document.getElementById('contact-name').value.trim();
    const address = document.getElementById('contact-address').value.trim();

    if (!fingerprint) {
      this.app.showNotification('Please enter a fingerprint', 'error');
      return;
    }

    if (!this.app.apiUrl) {
      this.app.showNotification('Not connected to Store', 'error');
      return;
    }

    try {
      const resp = await fetch(`${this.app.apiUrl}/contacts`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          fingerprint: fingerprint,
          display_name: name || fingerprint.slice(0, 12),
          store_address: address || '',
          trust_level: 1
        })
      });

      if (!resp.ok) throw new Error('Add failed');

      // Clear inputs
      document.getElementById('contact-fingerprint').value = '';
      document.getElementById('contact-name').value = '';
      document.getElementById('contact-address').value = '';

      // Reload contacts
      await this.loadContacts();

      this.app.showNotification('Contact added successfully', 'success');

    } catch (e) {
      console.error('Add contact failed:', e);
      this.app.showNotification('Failed to add contact', 'error');
    }
  }

  async deleteContact(fingerprint) {
    if (!confirm('Are you sure you want to delete this contact?')) return;

    try {
      const resp = await fetch(`${this.app.apiUrl}/contacts/${fingerprint}`, {
        method: 'DELETE'
      });

      if (!resp.ok) throw new Error('Delete failed');

      await this.loadContacts();
      this.app.showNotification('Contact deleted', 'success');

    } catch (e) {
      console.error('Delete contact failed:', e);
      this.app.showNotification('Failed to delete contact', 'error');
    }
  }

  getContactByFingerprint(fingerprint) {
    return this.contacts.find(c => c.fingerprint === fingerprint);
  }

  getContactDisplayName(fingerprint) {
    const contact = this.getContactByFingerprint(fingerprint);
    return contact ? (contact.display_name || fingerprint.slice(0, 12)) : fingerprint.slice(0, 12);
  }
}
