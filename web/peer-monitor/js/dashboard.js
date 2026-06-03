// 仪表板模块
class Dashboard {
    constructor(api) {
        this.api = api;
        this.updateInterval = null;
    }

    async init() {
        await this.update();
        this.startAutoUpdate();
    }

    async update() {
        try {
            const [stats, config] = await Promise.all([
                this.api.getStats(),
                this.api.getConfig()
            ]);

            this.updateStats(stats);
            this.updateSystemInfo(stats, config);
        } catch (error) {
            console.error('Dashboard update failed:', error);
        }
    }

    updateStats(stats) {
        document.getElementById('cachedMessages').textContent = stats.cached_messages || 0;
        document.getElementById('totalRelayed').textContent = stats.total_relayed || 0;
        document.getElementById('totalDelivered').textContent = stats.total_delivered || 0;
        document.getElementById('avgLatency').textContent =
            stats.avg_latency_ms ? `${stats.avg_latency_ms.toFixed(2)} ms` : '-';
        document.getElementById('activeConnections').textContent = stats.active_connections || 0;
        document.getElementById('errorCount').textContent = stats.error_count || 0;
    }

    updateSystemInfo(stats, config) {
        const uptime = this.formatUptime(stats.uptime_seconds || 0);
        document.getElementById('uptime').textContent = uptime;
        document.getElementById('listenAddr').textContent = config.listen_addr || '-';
        document.getElementById('maxCache').textContent = config.max_cache_entries || '-';
        document.getElementById('cacheTTL').textContent =
            config.cache_ttl_ms ? `${(config.cache_ttl_ms / 1000).toFixed(0)}s` : '-';
    }

    formatUptime(seconds) {
        const days = Math.floor(seconds / 86400);
        const hours = Math.floor((seconds % 86400) / 3600);
        const minutes = Math.floor((seconds % 3600) / 60);
        const secs = seconds % 60;

        if (days > 0) {
            return `${days}天 ${hours}小时`;
        } else if (hours > 0) {
            return `${hours}小时 ${minutes}分钟`;
        } else if (minutes > 0) {
            return `${minutes}分钟 ${secs}秒`;
        } else {
            return `${secs}秒`;
        }
    }

    startAutoUpdate() {
        this.updateInterval = setInterval(() => this.update(), 2000);
    }

    stop() {
        if (this.updateInterval) {
            clearInterval(this.updateInterval);
            this.updateInterval = null;
        }
    }
}
