// NextIM Web Chat - Rooms Module
class RoomsModule {
  constructor(app) {
    this.app = app;
    this.rooms = new Map(); // roomId -> room data

    this.setupEventListeners();
  }

  setupEventListeners() {
    // Create room button
    document.getElementById('create-room-btn').addEventListener('click', () => this.createRoom());

    // Enter key in room input
    document.getElementById('room-name-input').addEventListener('keydown', (e) => {
      if (e.key === 'Enter') this.createRoom();
    });

    // Join room button
    document.getElementById('join-room-btn').addEventListener('click', () => this.showJoinRoomModal());
  }

  async loadRooms() {
    if (!this.app.apiUrl) return;

    try {
      const resp = await fetch(`${this.app.apiUrl}/rooms`);
      if (!resp.ok) throw new Error('Load failed');

      const serverRooms = await resp.json();

      // Update rooms map
      for (const room of serverRooms) {
        if (!this.rooms.has(room.room_id)) {
          this.rooms.set(room.room_id, {
            room_id: room.room_id,
            name: room.name,
            memberCount: room.member_count,
            encrypted: room.encrypted,
            lastMessage: '',
            lastMessageTime: 0,
            unreadCount: 0
          });
        } else {
          // Update existing room data
          const existing = this.rooms.get(room.room_id);
          existing.memberCount = room.member_count;
          existing.encrypted = room.encrypted;
        }
      }

      this.renderRooms();

    } catch (e) {
      console.error('Load rooms failed:', e);
      this.renderEmptyState('Failed to load rooms');
    }
  }

  renderRooms() {
    const container = document.getElementById('room-list');

    if (this.rooms.size === 0) {
      this.renderEmptyState('No rooms yet. Create your first room below!');
      return;
    }

    // Sort rooms by last message time
    const sortedRooms = Array.from(this.rooms.values()).sort((a, b) => {
      return b.lastMessageTime - a.lastMessageTime;
    });

    container.innerHTML = sortedRooms.map(room => this.renderRoom(room)).join('');

    // Add click handlers
    container.querySelectorAll('.room-item').forEach(item => {
      item.addEventListener('click', () => {
        const roomId = item.dataset.roomId;
        const room = this.rooms.get(roomId);
        this.selectRoom(roomId, room.name);
      });
    });
  }

  renderRoom(room) {
    const isActive = this.app.chat.currentRoom === room.room_id;
    const initials = this.app.getInitials(room.name);
    const avatarColor = this.app.getAvatarColor(room.room_id);
    const time = room.lastMessageTime ? this.app.formatTime(room.lastMessageTime) : '';

    return `
      <div class="room-item ${isActive ? 'active' : ''}" data-room-id="${room.room_id}">
        <div class="item-avatar" style="background: ${avatarColor}">
          ${initials}
        </div>
        <div class="item-content">
          <div class="item-header">
            <div class="item-name">${this.app.escapeHtml(room.name)}</div>
            ${time ? `<div class="item-time">${time}</div>` : ''}
          </div>
          <div class="item-preview">
            ${room.lastMessage ? this.app.escapeHtml(room.lastMessage.slice(0, 40)) : 'No messages yet'}
          </div>
        </div>
        ${room.unreadCount > 0 ? `<div class="unread-badge">${room.unreadCount}</div>` : ''}
      </div>
    `;
  }

  renderEmptyState(message) {
    const container = document.getElementById('room-list');
    container.innerHTML = `
      <div class="empty-list">
        <div style="font-size:32px;margin-bottom:8px">💬</div>
        <div>${message}</div>
      </div>
    `;
  }

  selectRoom(roomId, roomName) {
    // Clear unread count
    const room = this.rooms.get(roomId);
    if (room) {
      room.unreadCount = 0;
    }

    // Update UI
    this.renderRooms();

    // Tell chat module to load this room
    this.app.chat.selectRoom(roomId, roomName);
  }

  async createRoom() {
    const input = document.getElementById('room-name-input');
    const name = input.value.trim();

    if (!name) {
      this.app.showNotification('Please enter a room name', 'error');
      return;
    }

    if (!this.app.apiUrl) {
      this.app.showNotification('Not connected to Store', 'error');
      return;
    }

    try {
      const resp = await fetch(`${this.app.apiUrl}/rooms`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: name,
          creator_fingerprint: this.app.username,
          room_type: 'group'
        })
      });

      if (!resp.ok) throw new Error('Create failed');

      const room = await resp.json();

      // Add to rooms map
      this.rooms.set(room.room_id, {
        room_id: room.room_id,
        name: room.name,
        memberCount: room.member_count || 1,
        encrypted: room.encrypted || false,
        lastMessage: '',
        lastMessageTime: Date.now(),
        unreadCount: 0
      });

      // Clear input
      input.value = '';

      // Render and select new room
      this.renderRooms();
      this.selectRoom(room.room_id, room.name);

      this.app.showNotification('Room created successfully', 'success');

    } catch (e) {
      console.error('Create room failed:', e);
      this.app.showNotification('Failed to create room', 'error');
    }
  }

  async joinRoom(roomId) {
    if (!this.app.apiUrl) return;

    try {
      const resp = await fetch(`${this.app.apiUrl}/rooms/${roomId}/join`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          user_fingerprint: this.app.username
        })
      });

      if (!resp.ok) throw new Error('Join failed');

      await this.loadRooms();
      this.app.showNotification('Joined room successfully', 'success');

    } catch (e) {
      console.error('Join room failed:', e);
      this.app.showNotification('Failed to join room', 'error');
    }
  }

  showJoinRoomModal() {
    // Simple prompt for now
    const roomId = prompt('Enter room ID to join:');
    if (roomId) {
      this.joinRoom(roomId.trim());
    }
  }

  updateRoomPreview(roomId, message) {
    const room = this.rooms.get(roomId);
    if (room) {
      room.lastMessage = message;
      room.lastMessageTime = Date.now();

      // Increment unread if not current room
      if (this.app.chat.currentRoom !== roomId) {
        room.unreadCount = (room.unreadCount || 0) + 1;
      }

      this.renderRooms();
    }
  }

  getRoomName(roomId) {
    const room = this.rooms.get(roomId);
    return room ? room.name : roomId;
  }
}
