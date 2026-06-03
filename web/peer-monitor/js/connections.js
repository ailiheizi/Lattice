// 连接管理模块
class ConnectionManager {
    constructor(api) {
        this.api = api;
        this.updateInterval = null;
    }

    async init() {
        await this.update();
    }

    async update() {
        try {
            const data = await this.api.getConnections();
            this.updateActiveConnections(data.active || []);
            this.updateConnectionHistory(data.history || []);
        } catch (error) {
            console.error('Connections update failed:', error);
        }
    }

    updateActiveConnections(connections) {
        const tbody = document.getElementById('activeConnectionsTable');
        if (!tbody) return;

        if (connections.length === 0) {
            tbody.innerHTML = '<tr><td colspan="5">暂无活跃连接</td></tr>';
            return;
        }

        tbody.innerHTML = connections.map(conn => `
            <tr>
                <td>${this.truncate(conn.id, 16)}</td>
                <td>${conn.remote_addr}</td>
                <td>${this.formatTimestamp(conn.connected_at)}</td>
                <td>${conn.message_count}</td>
                <td><span style="color: var(--success-color);">${conn.status}</span></td>
            </tr>
        `).join('');
    }

    updateConnectionHistory(history) {
        const tbody = document.getElementById('connectionHistoryTable');
        if (!tbody) return;

        if (history.length === 0) {
            tbody.innerHTML = '<tr><td colspan="5">暂无历史记录</td></tr>';
            return;
        }

        tbody.innerHTML = history.map(conn => `
            <tr>
                <td>${conn.remote_addr}</td>
                <td>${this.formatTimestamp(conn.connected_at)}</td>
                <td>${this.formatTimestamp(conn.disconnected_at)}</td>
                <td>${this.formatDuration(conn.duration_seconds)}</td>
                <td>${conn.message_count}</td>
            </tr>
        `).join('');
    }

    formatTimestamp(timestamp) {
        if (!timestamp) return '-';
        const date = new Date(timestamp * 1000);
        return date.toLocaleString('zh-CN');
    }

    formatDuration(seconds) {
        if (!seconds) return '-';
        const hours = Math.floor(seconds / 3600);
        const minutes = Math.floor((seconds % 3600) / 60);
        const secs = seconds % 60;

        if (hours > 0) {
            return `${hours}h ${minutes}m`;
        } else if (minutes > 0) {
            return `${minutes}m ${secs}s`;
        } else {
            return `${secs}s`;
        }
    }

    truncate(str, length) {
        if (!str) return '-';
        return str.length > length ? str.substring(0, length) + '...' : str;
    }

    startAutoUpdate() {
        this.updateInterval = setInterval(() => this.update(), 5000);
    }

    stop() {
        if (this.updateInterval) {
            clearInterval(this.updateInterval);
            this.updateInterval = null;
        }
    }
}

async function refreshConnections() {
    if (window.connectionManager) {
        await window.connectionManager.update();
    }
}
