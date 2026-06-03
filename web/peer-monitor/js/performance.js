// 性能监控模块
class PerformanceMonitor {
    constructor(api) {
        this.api = api;
        this.updateInterval = null;
        this.latencyHistory = [];
        this.throughputHistory = [];
        this.maxHistoryLength = 60;
    }

    async init() {
        await this.update();
        this.startAutoUpdate();
    }

    async update() {
        try {
            const stats = await this.api.getStats();
            this.updatePerformanceTable(stats);
            this.updateHistoryData(stats);
        } catch (error) {
            console.error('Performance update failed:', error);
        }
    }

    updatePerformanceTable(stats) {
        const tbody = document.getElementById('perfStatsTable');
        if (!tbody) return;

        const metrics = [
            {
                name: '消息中转速率',
                current: `${stats.total_relayed || 0}/s`,
                avg: '-',
                max: '-',
                min: '-'
            },
            {
                name: '投递速率',
                current: `${stats.total_delivered || 0}/s`,
                avg: '-',
                max: '-',
                min: '-'
            },
            {
                name: '延迟',
                current: stats.avg_latency_ms ? `${stats.avg_latency_ms.toFixed(2)} ms` : '-',
                avg: '-',
                max: '-',
                min: '-'
            },
            {
                name: '缓存使用率',
                current: stats.cached_messages ?
                    `${((stats.cached_messages / 10000) * 100).toFixed(1)}%` : '0%',
                avg: '-',
                max: '-',
                min: '-'
            }
        ];

        tbody.innerHTML = metrics.map(m => `
            <tr>
                <td>${m.name}</td>
                <td>${m.current}</td>
                <td>${m.avg}</td>
                <td>${m.max}</td>
                <td>${m.min}</td>
            </tr>
        `).join('');
    }

    updateHistoryData(stats) {
        const timestamp = Date.now();

        this.latencyHistory.push({
            time: timestamp,
            value: stats.avg_latency_ms || 0
        });

        this.throughputHistory.push({
            time: timestamp,
            relayed: stats.total_relayed || 0,
            delivered: stats.total_delivered || 0
        });

        if (this.latencyHistory.length > this.maxHistoryLength) {
            this.latencyHistory.shift();
        }
        if (this.throughputHistory.length > this.maxHistoryLength) {
            this.throughputHistory.shift();
        }
    }

    getLatencyData() {
        return this.latencyHistory;
    }

    getThroughputData() {
        return this.throughputHistory;
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
