// Dashboard Module
class Dashboard {
    constructor() {
        this.refreshTimer = null;
        this.isActive = false;
    }

    async init() {
        this.isActive = true;
        await this.loadStats();
        this.startAutoRefresh();
    }

    destroy() {
        this.isActive = false;
        this.stopAutoRefresh();
    }

    async loadStats() {
        try {
            showLoading('dashboard-content');
            const stats = await api.getStats();
            this.renderStats(stats);
            this.renderCharts(stats);
            hideLoading('dashboard-content');
        } catch (error) {
            hideLoading('dashboard-content');
            showError('dashboard-content', 'Failed to load statistics: ' + error.message);
        }
    }

    renderStats(stats) {
        // Update stat cards
        this.updateStatCard('total-messages', stats.total_messages || 0);
        this.updateStatCard('total-contacts', stats.total_contacts || 0);
        this.updateStatCard('total-rooms', stats.total_rooms || 0);
        this.updateStatCard('storage-used', formatBytes(stats.storage_used || 0));

        // Update storage percentage
        const storagePercent = stats.storage_total > 0
            ? ((stats.storage_used / stats.storage_total) * 100).toFixed(1)
            : 0;

        const storageBar = document.querySelector('.storage-bar');
        if (storageBar) {
            storageBar.style.width = `${storagePercent}%`;
            storageBar.className = `storage-bar ${this.getStorageClass(storagePercent)}`;
        }

        const storageText = document.querySelector('.storage-text');
        if (storageText) {
            storageText.textContent = `${formatBytes(stats.storage_used)} / ${formatBytes(stats.storage_total)} (${storagePercent}%)`;
        }
    }

    updateStatCard(id, value) {
        const element = document.getElementById(id);
        if (element) {
            element.textContent = value;
        }
    }

    getStorageClass(percent) {
        if (percent >= 90) return 'bg-danger';
        if (percent >= 75) return 'bg-warning';
        return 'bg-success';
    }

    renderCharts(stats) {
        // Message trend chart
        const messageTrend = this.generateTrendData(stats.message_trend || []);
        chartManager.createMessageChart('messageChart', messageTrend);

        // Storage chart
        chartManager.createStorageChart('storageChart',
            stats.storage_used || 0,
            stats.storage_total || 1
        );

        // Activity chart
        const activityData = this.generateActivityData(stats.recent_activity || []);
        chartManager.createActivityChart('activityChart', activityData);
    }

    generateTrendData(trend) {
        if (trend.length === 0) {
            // Generate dummy data for last 7 days
            const labels = [];
            const values = [];
            for (let i = 6; i >= 0; i--) {
                const date = new Date();
                date.setDate(date.getDate() - i);
                labels.push(date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' }));
                values.push(0);
            }
            return { labels, values };
        }
        return trend;
    }

    generateActivityData(activity) {
        if (activity.length === 0) {
            // Generate dummy data for last 24 hours
            const labels = [];
            const values = [];
            for (let i = 23; i >= 0; i--) {
                labels.push(`${i}:00`);
                values.push(0);
            }
            return { labels, values };
        }
        return activity;
    }

    startAutoRefresh() {
        this.stopAutoRefresh();
        this.refreshTimer = setInterval(() => {
            if (this.isActive) {
                this.loadStats();
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

window.dashboard = new Dashboard();
