// API 调用模块
class PeerAPI {
    constructor(baseURL = '') {
        this.baseURL = baseURL;
    }

    async request(endpoint) {
        try {
            const response = await fetch(`${this.baseURL}${endpoint}`);
            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }
            return await response.json();
        } catch (error) {
            console.error(`API request failed: ${endpoint}`, error);
            throw error;
        }
    }

    async health() {
        const response = await fetch(`${this.baseURL}/health`);
        return response.ok;
    }

    async getStats() {
        return this.request('/stats');
    }

    async getConnections() {
        return this.request('/connections');
    }

    async getCache() {
        return this.request('/cache');
    }

    async getConfig() {
        return this.request('/config');
    }
}

// 导出单例
const api = new PeerAPI('http://localhost:9201');
