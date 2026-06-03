// Contacts Module
class Contacts {
    constructor() {
        this.currentContacts = [];
        this.refreshTimer = null;
        this.isActive = false;
    }

    async init() {
        this.isActive = true;
        this.setupEventListeners();
        await this.loadContacts();
        this.startAutoRefresh();
    }

    destroy() {
        this.isActive = false;
        this.stopAutoRefresh();
    }

    setupEventListeners() {
        const searchInput = document.getElementById('contact-search');
        if (searchInput) {
            searchInput.addEventListener('input', (e) => this.filterContacts(e.target.value));
        }

        const refreshBtn = document.getElementById('refresh-contacts');
        if (refreshBtn) {
            refreshBtn.addEventListener('click', () => this.loadContacts());
        }
    }

    async loadContacts() {
        try {
            showLoading('contacts-list');
            const contacts = await api.getContacts();
            this.currentContacts = contacts;
            this.renderContacts(contacts);
            hideLoading('contacts-list');
        } catch (error) {
            hideLoading('contacts-list');
            showError('contacts-list', 'Failed to load contacts: ' + error.message);
        }
    }

    renderContacts(contacts) {
        const container = document.getElementById('contacts-list');
        if (!container) return;

        if (contacts.length === 0) {
            container.innerHTML = '<div class="empty-state">No contacts found</div>';
            return;
        }

        container.innerHTML = contacts.map(contact => this.createContactCard(contact)).join('');

        // Attach event handlers
        container.querySelectorAll('.view-messages').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const contactId = e.target.closest('.view-messages').dataset.id;
                this.viewContactMessages(contactId);
            });
        });

        container.querySelectorAll('.delete-contact').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const contactId = e.target.closest('.delete-contact').dataset.id;
                this.deleteContact(contactId);
            });
        });
    }

    createContactCard(contact) {
        const lastSeen = contact.last_seen
            ? new Date(contact.last_seen).toLocaleString()
            : 'Never';

        const messageCount = contact.message_count || 0;
        const status = this.getContactStatus(contact.last_seen);

        return `
            <div class="contact-card" data-id="${contact.id || contact.contact_id}">
                <div class="contact-header">
                    <div class="contact-avatar">${this.getInitials(contact.name || contact.id)}</div>
                    <div class="contact-info">
                        <div class="contact-name">${this.escapeHtml(contact.name || contact.id)}</div>
                        <div class="contact-id">${this.escapeHtml(contact.id || '')}</div>
                    </div>
                    <span class="contact-status status-${status}">${status}</span>
                </div>
                <div class="contact-stats">
                    <div class="stat">
                        <span class="stat-label">Messages</span>
                        <span class="stat-value">${messageCount}</span>
                    </div>
                    <div class="stat">
                        <span class="stat-label">Last Seen</span>
                        <span class="stat-value">${lastSeen}</span>
                    </div>
                </div>
                <div class="contact-actions">
                    <button class="btn btn-sm btn-primary view-messages" data-id="${contact.id || contact.contact_id}">
                        View Messages
                    </button>
                    <button class="btn btn-sm btn-danger delete-contact" data-id="${contact.id || contact.contact_id}">
                        Delete
                    </button>
                </div>
            </div>
        `;
    }

    getInitials(name) {
        if (!name) return '?';
        const parts = name.split(/[\s_-]+/);
        if (parts.length >= 2) {
            return (parts[0][0] + parts[1][0]).toUpperCase();
        }
        return name.substring(0, 2).toUpperCase();
    }

    getContactStatus(lastSeen) {
        if (!lastSeen) return 'offline';

        const now = Date.now();
        const lastSeenTime = new Date(lastSeen).getTime();
        const diffMinutes = (now - lastSeenTime) / (1000 * 60);

        if (diffMinutes < 5) return 'online';
        if (diffMinutes < 30) return 'away';
        return 'offline';
    }

    async viewContactMessages(contactId) {
        // Switch to messages tab and filter by contact
        app.switchPage('messages');
        await messages.loadMessages(contactId);
    }

    async deleteContact(contactId) {
        if (!confirm('Are you sure you want to delete this contact? This will also delete all associated messages.')) {
            return;
        }

        try {
            await api.deleteContact(contactId);
            showNotification('Contact deleted successfully', 'success');
            await this.loadContacts();
        } catch (error) {
            showNotification('Failed to delete contact: ' + error.message, 'error');
        }
    }

    filterContacts(query) {
        const filtered = this.currentContacts.filter(contact => {
            const searchText = `${contact.name} ${contact.id}`.toLowerCase();
            return searchText.includes(query.toLowerCase());
        });
        this.renderContacts(filtered);
    }

    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    startAutoRefresh() {
        this.stopAutoRefresh();
        this.refreshTimer = setInterval(() => {
            if (this.isActive) {
                this.loadContacts();
            }
        }, CONFIG.REFRESH_INTERVAL);
    }

    stopAutoRefresh() {
        if (this.refreshTimer) {
            clearInterval(this.refreshTimer);
            this.refreshTimer = null;
        }
    }
}

window.contacts = new Contacts();
