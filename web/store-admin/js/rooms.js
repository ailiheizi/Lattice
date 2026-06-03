// Rooms Module
class Rooms {
    constructor() {
        this.currentRooms = [];
        this.refreshTimer = null;
        this.isActive = false;
    }

    async init() {
        this.isActive = true;
        this.setupEventListeners();
        await this.loadRooms();
        this.startAutoRefresh();
    }

    destroy() {
        this.isActive = false;
        this.stopAutoRefresh();
    }

    setupEventListeners() {
        const searchInput = document.getElementById('room-search');
        if (searchInput) {
            searchInput.addEventListener('input', (e) => this.filterRooms(e.target.value));
        }

        const refreshBtn = document.getElementById('refresh-rooms');
        if (refreshBtn) {
            refreshBtn.addEventListener('click', () => this.loadRooms());
        }
    }

    async loadRooms() {
        try {
            showLoading('rooms-list');
            const rooms = await api.getRooms();
            this.currentRooms = rooms;
            this.renderRooms(rooms);
            hideLoading('rooms-list');
        } catch (error) {
            hideLoading('rooms-list');
            showError('rooms-list', 'Failed to load rooms: ' + error.message);
        }
    }

    renderRooms(rooms) {
        const container = document.getElementById('rooms-list');
        if (!container) return;

        if (rooms.length === 0) {
            container.innerHTML = '<div class="empty-state">No rooms found</div>';
            return;
        }

        container.innerHTML = rooms.map(room => this.createRoomCard(room)).join('');

        // Attach event handlers
        container.querySelectorAll('.view-room-messages').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const roomId = e.target.closest('.view-room-messages').dataset.id;
                this.viewRoomMessages(roomId);
            });
        });

        container.querySelectorAll('.delete-room').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const roomId = e.target.closest('.delete-room').dataset.id;
                this.deleteRoom(roomId);
            });
        });
    }

    createRoomCard(room) {
        const createdAt = room.created_at
            ? new Date(room.created_at).toLocaleString()
            : 'Unknown';

        const memberCount = room.members ? room.members.length : 0;
        const messageCount = room.message_count || 0;

        return `
            <div class="room-card" data-id="${room.id || room.room_id}">
                <div class="room-header">
                    <div class="room-icon">
                        <svg width="24" height="24" fill="currentColor" viewBox="0 0 20 20">
                            <path d="M2 5a2 2 0 012-2h7a2 2 0 012 2v4a2 2 0 01-2 2H9l-3 3v-3H4a2 2 0 01-2-2V5z"/>
                            <path d="M15 7v2a4 4 0 01-4 4H9.828l-1.766 1.767c.28.149.599.233.938.233h2l3 3v-3h2a2 2 0 002-2V9a2 2 0 00-2-2h-1z"/>
                        </svg>
                    </div>
                    <div class="room-info">
                        <div class="room-name">${this.escapeHtml(room.name || room.id)}</div>
                        <div class="room-id">${this.escapeHtml(room.id || '')}</div>
                    </div>
                </div>
                <div class="room-stats">
                    <div class="stat">
                        <span class="stat-label">Members</span>
                        <span class="stat-value">${memberCount}</span>
                    </div>
                    <div class="stat">
                        <span class="stat-label">Messages</span>
                        <span class="stat-value">${messageCount}</span>
                    </div>
                    <div class="stat">
                        <span class="stat-label">Created</span>
                        <span class="stat-value">${createdAt}</span>
                    </div>
                </div>
                ${this.renderMembers(room.members)}
                <div class="room-actions">
                    <button class="btn btn-sm btn-primary view-room-messages" data-id="${room.id || room.room_id}">
                        View Messages
                    </button>
                    <button class="btn btn-sm btn-danger delete-room" data-id="${room.id || room.room_id}">
                        Delete
                    </button>
                </div>
            </div>
        `;
    }

    renderMembers(members) {
        if (!members || members.length === 0) {
            return '<div class="room-members">No members</div>';
        }

        const memberList = members.slice(0, 5).map(m =>
            `<span class="member-badge">${this.escapeHtml(m)}</span>`
        ).join('');

        const moreCount = members.length > 5 ? members.length - 5 : 0;
        const moreText = moreCount > 0 ? `<span class="member-badge">+${moreCount} more</span>` : '';

        return `
            <div class="room-members">
                <div class="members-label">Members:</div>
                <div class="members-list">${memberList}${moreText}</div>
            </div>
        `;
    }

    async viewRoomMessages(roomId) {
        // Switch to messages tab and filter by room
        app.switchPage('messages');
        await messages.loadMessages(roomId);
    }

    async deleteRoom(roomId) {
        if (!confirm('Are you sure you want to delete this room? This will also delete all associated messages.')) {
            return;
        }

        try {
            await api.deleteRoom(roomId);
            showNotification('Room deleted successfully', 'success');
            await this.loadRooms();
        } catch (error) {
            showNotification('Failed to delete room: ' + error.message, 'error');
        }
    }

    filterRooms(query) {
        const filtered = this.currentRooms.filter(room => {
            const searchText = `${room.name} ${room.id}`.toLowerCase();
            return searchText.includes(query.toLowerCase());
        });
        this.renderRooms(filtered);
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
                this.loadRooms();
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

window.rooms = new Rooms();
