// API Client for NextIM Store
class StoreAPI {
    constructor(baseURL) {
        this.baseURL = baseURL;
    }

    async request(endpoint, options = {}) {
        const url = `${this.baseURL}${endpoint}`;
        try {
            const response = await fetch(url, {
                ...options,
                headers: {
                    'Content-Type': 'application/json',
                    ...options.headers
                }
            });

            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }

            return await response.json();
        } catch (error) {
            console.error(`API Error [${endpoint}]:`, error);
            throw error;
        }
    }

    // GET request
    async get(endpoint) {
        return this.request(endpoint, { method: 'GET' });
    }

    // POST request
    async post(endpoint, data) {
        return this.request(endpoint, {
            method: 'POST',
            body: JSON.stringify(data)
        });
    }

    // DELETE request
    async delete(endpoint) {
        return this.request(endpoint, { method: 'DELETE' });
    }

    // Stats API
    async getStats() {
        return this.get(API_ENDPOINTS.STATS);
    }

    // Messages API
    async getMessages(limit = 100) {
        return this.get(`${API_ENDPOINTS.MESSAGES}?limit=${limit}`);
    }

    async getMessagesByContact(contactId, limit = 100) {
        return this.get(`${API_ENDPOINTS.MESSAGES}/${contactId}?limit=${limit}`);
    }

    async deleteMessage(messageId) {
        return this.delete(`${API_ENDPOINTS.MESSAGES}/${messageId}`);
    }

    // Contacts API
    async getContacts() {
        return this.get(API_ENDPOINTS.CONTACTS);
    }

    async getContact(contactId) {
        return this.get(`${API_ENDPOINTS.CONTACTS}/${contactId}`);
    }

    async deleteContact(contactId) {
        return this.delete(`${API_ENDPOINTS.CONTACTS}/${contactId}`);
    }

    // Rooms API
    async getRooms() {
        return this.get(API_ENDPOINTS.ROOMS);
    }

    async getRoom(roomId) {
        return this.get(`${API_ENDPOINTS.ROOMS}/${roomId}`);
    }

    async deleteRoom(roomId) {
        return this.delete(`${API_ENDPOINTS.ROOMS}/${roomId}`);
    }

    // Logs API (if available)
    async getLogs(limit = 100) {
        try {
            return this.get(`${API_ENDPOINTS.LOGS}?limit=${limit}`);
        } catch (error) {
            // Logs endpoint might not exist
            return [];
        }
    }
}

// Create global API instance
window.api = new StoreAPI(CONFIG.API_BASE_URL);
