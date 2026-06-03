// Messages Module
class Messages {
    constructor() {
        this.currentMessages = [];
        this.selectedContact = null;
        this.refreshTimer = null;
        this.isActive = false;
    }

    async init() {
        this.isActive = true;
        this.setupEventListeners();
        await this.loadMessages();
        this.startAutoRefresh();
    }

    destroy() {
        this.isActive = false;
        this.stopAutoRefresh();
    }

    setupEventListeners() {
        const searchInput = document.getElementById('message-search');
        if (searchInput) {
            searchInput.addEventListener('input', (e) => this.filterMessages(e.target.value));
        }

        const refreshBtn = document.getElementById('refresh-messages');
        if (refreshBtn) {
            refreshBtn.addEventListener('click', () => this.loadMessages());
        }
    }

    async loadMessages(contactId = null) {
        try {
            showLoading('messages-list');

            const messages = contactId
                ? await api.getMessagesByContact(contactId)
                : await api.getMessages();

            this.currentMessages = messages;
            this.renderMessages(messages);
            hideLoading('messages-list');
        } catch (error) {
            hideLoading('messages-list');
            showError('messages-list', 'Failed to load messages: ' + error.message);
        }
    }

    renderMessages(messages) {
        const container = document.getElementById('messages-list');
        if (!container) return;

        if (messages.length === 0) {
            container.innerHTML = '<div class="empty-state">No messages found</div>';
            return;
        }

        container.innerHTML = messages.map(msg => this.createMessageCard(msg)).join('');

        // Attach delete handlers
        container.querySelectorAll('.delete-message').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const msgId = e.target.closest('.delete-message').dataset.id;
                this.deleteMessage(msgId);
            });
        });
    }

    createMessageCard(msg) {
        const timestamp = new Date(msg.timestamp || Date.now()).toLocaleString();
        const contentPreview = this.truncateText(msg.content || '', 100);
        const messageType = msg.message_type || 'text';

        return `
            <div class="message-card" data-id="${msg.id || msg.message_id}">
                <div class="message-header">
                    <div class="message-info">
                        <span class="message-from">${this.escapeHtml(msg.from || 'Unknown')}</span>
                        <span class="message-arrow">→</span>
                        <span class="message-to">${this.escapeHtml(msg.to || 'Unknown')}</span>
                    </div>
                    <span class="message-type badge badge-${this.getTypeColor(messageType)}">${messageType}</span>
                </div>
                <div class="message-content">${this.escapeHtml(contentPreview)}</div>
                <div class="message-footer">
                    <span class="message-time">${timestamp}</span>
                    <button class="btn btn-sm btn-danger delete-message" data-id="${msg.id || msg.message_id}">
                        Delete
                    </button>
                </div>
            </div>
        `;
    }

    getTypeColor(type) {
        const colors = {
            'text': 'primary',
            'image': 'success',
            'file': 'info',
            'audio': 'warning',
            'video': 'danger'
        };
        return colors[type] || 'secondary';
    }

    async deleteMessage(messageId) {
        if (!confirm('Are you sure you want to delete this message?')) {
            return;
        }

        try {
            await api.deleteMessage(messageId);
            showNotification('Message deleted successfully', 'success');
            await this.loadMessages();
        } catch (error) {
            showNotification('Failed to delete message: ' + error.message, 'error');
        }
    }

    filterMessages(query) {
        const filtered = this.currentMessages.filter(msg => {
            const searchText = `${msg.from} ${msg.to} ${msg.content}`.toLowerCase();
            return searchText.includes(query.toLowerCase());
        });
        this.renderMessages(filtered);
    }

    truncateText(text, maxLength) {
        if (text.length <= maxLength) return text;
        return text.substring(0, maxLength) + '...';
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
                this.loadMessages();
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

window.messages = new Messages();
