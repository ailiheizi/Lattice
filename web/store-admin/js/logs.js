// Logs Module
class Logs {
    constructor() {
        this.currentLogs = [];
        this.refreshTimer = null;
        this.isActive = false;
        this.autoScroll = true;
    }

    async init() {
        this.isActive = true;
        this.setupEventListeners();
        await this.loadLogs();
        this.startAutoRefresh();
    }

    destroy() {
        this.isActive = false;
        this.stopAutoRefresh();
    }

    setupEventListeners() {
        const searchInput = document.getElementById('log-search');
        if (searchInput) {
            searchInput.addEventListener('input', (e) => this.filterLogs(e.target.value));
        }

        const refreshBtn = document.getElementById('refresh-logs');
        if (refreshBtn) {
            refreshBtn.addEventListener('click', () => this.loadLogs());
        }

        const clearBtn = document.getElementById('clear-logs');
        if (clearBtn) {
            clearBtn.addEventListener('click', () => this.clearLogs());
        }

        const levelFilter = document.getElementById('log-level-filter');
        if (levelFilter) {
            levelFilter.addEventListener('change', (e) => this.filterByLevel(e.target.value));
        }

        const autoScrollToggle = document.getElementById('auto-scroll-toggle');
        if (autoScrollToggle) {
            autoScrollToggle.addEventListener('change', (e) => {
                this.autoScroll = e.target.checked;
            });
        }
    }

    async loadLogs() {
        try {
            showLoading('logs-container');
            const logs = await api.getLogs();
            this.currentLogs = logs;
            this.renderLogs(logs);
            hideLoading('logs-container');
        } catch (error) {
            hideLoading('logs-container');
            // Logs endpoint might not exist, show placeholder
            this.renderPlaceholder();
        }
    }

    renderLogs(logs) {
        const container = document.getElementById('logs-container');
        if (!container) return;

        if (logs.length === 0) {
            container.innerHTML = '<div class="empty-state">No logs available</div>';
            return;
        }

        container.innerHTML = logs.map(log => this.createLogEntry(log)).join('');

        if (this.autoScroll) {
            container.scrollTop = container.scrollHeight;
        }
    }

    createLogEntry(log) {
        const timestamp = log.timestamp
            ? new Date(log.timestamp).toLocaleString()
            : new Date().toLocaleString();

        const level = log.level || 'info';
        const message = log.message || log.msg || '';
        const source = log.source || log.module || 'system';

        return `
            <div class="log-entry log-${level}">
                <span class="log-timestamp">${timestamp}</span>
                <span class="log-level badge badge-${this.getLevelColor(level)}">${level.toUpperCase()}</span>
                <span class="log-source">[${this.escapeHtml(source)}]</span>
                <span class="log-message">${this.escapeHtml(message)}</span>
            </div>
        `;
    }

    getLevelColor(level) {
        const colors = {
            'error': 'danger',
            'warn': 'warning',
            'warning': 'warning',
            'info': 'info',
            'debug': 'secondary',
            'trace': 'secondary'
        };
        return colors[level.toLowerCase()] || 'secondary';
    }

    renderPlaceholder() {
        const container = document.getElementById('logs-container');
        if (!container) return;

        container.innerHTML = `
            <div class="empty-state">
                <p>Log endpoint not available</p>
                <p class="text-muted">The Store server may not have logging enabled</p>
            </div>
        `;
    }

    filterLogs(query) {
        const filtered = this.currentLogs.filter(log => {
            const searchText = `${log.level} ${log.source} ${log.message}`.toLowerCase();
            return searchText.includes(query.toLowerCase());
        });
        this.renderLogs(filtered);
    }

    filterByLevel(level) {
        if (level === 'all') {
            this.renderLogs(this.currentLogs);
            return;
        }

        const filtered = this.currentLogs.filter(log =>
            log.level && log.level.toLowerCase() === level.toLowerCase()
        );
        this.renderLogs(filtered);
    }

    clearLogs() {
        const container = document.getElementById('logs-container');
        if (container) {
            container.innerHTML = '<div class="empty-state">Logs cleared</div>';
        }
        this.currentLogs = [];
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
                this.loadLogs();
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

window.logs = new Logs();
