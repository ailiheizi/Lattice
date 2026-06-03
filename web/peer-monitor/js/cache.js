// 缓存管理模块
class CacheManager {
    constructor(api) {
        this.api = api;
        this.updateInterval = null;
    }

    async init() {
        await this.update();
    }

    async update() {
        try {
            const data = await this.api.getCache();
            this.updateCacheStats(data.stats);
            this.updateCacheEntries(data.entries || []);
        } catch (error) {
            console.error('Cache update failed:', error);
        }
    }

    updateCacheStats(stats) {
        const entriesEl = document.getElementById('cacheEntries');
        const usageEl = document.getElementById('cacheUsage');
        const oldestEl = document.getElementById('oldestEntry');

        if (entriesEl) {
            entriesEl.textContent = stats.total_entries || 0;
        }

        if (usageEl && stats.max_entries) {
            const usage = ((stats.total_entries / stats.max_entries) * 100).toFixed(1);
            usageEl.textContent = `${usage}%`;
        }

        if (oldestEl && stats.total_entries > 0) {
            // 找到最旧的条目
            oldestEl.textContent = '-';
        } else if (oldestEl) {
            oldestEl.textContent = '-';
        }
    }

    updateCacheEntries(entries) {
        const tbody = document.getElementById('cacheEntriesTable');
        if (!tbody) return;

        if (entries.length === 0) {
            tbody.innerHTML = '<tr><td colspan="5">缓存为空</td></tr>';
            return;
        }

        // 按接收方分组
        const grouped = this.groupByRecipient(entries);

        tbody.innerHTML = Object.entries(grouped).map(([recipient, items]) => {
            const count = items.length;
            const oldest = Math.max(...items.map(e => e.age_ms));
            const newest = Math.min(...items.map(e => e.age_ms));
            const hasExpired = items.some(e => e.is_expired);

            return `
                <tr>
                    <td title="${recipient}">${this.truncate(recipient, 32)}</td>
                    <td>${count}</td>
                    <td>${this.formatAge(oldest)}</td>
                    <td>${this.formatAge(newest)}</td>
                    <td>
                        ${hasExpired ? '<span style="color: var(--warning-color);">已过期</span>' : ''}
                        <button class="btn btn-sm btn-danger" onclick="clearRecipientCache('${recipient}')">清除</button>
                    </td>
                </tr>
            `;
        }).join('');
    }

    groupByRecipient(entries) {
        const grouped = {};
        for (const entry of entries) {
            const recipient = entry.recipient_fingerprint;
            if (!grouped[recipient]) {
                grouped[recipient] = [];
            }
            grouped[recipient].push(entry);
        }
        return grouped;
    }

    formatAge(ageMs) {
        if (!ageMs) return '-';
        const seconds = Math.floor(ageMs / 1000);
        const minutes = Math.floor(seconds / 60);
        const hours = Math.floor(minutes / 60);

        if (hours > 0) {
            return `${hours}小时前`;
        } else if (minutes > 0) {
            return `${minutes}分钟前`;
        } else {
            return `${seconds}秒前`;
        }
    }

    truncate(str, length) {
        if (!str) return '-';
        return str.length > length ? str.substring(0, length) + '...' : str;
    }

    startAutoUpdate() {
        this.updateInterval = setInterval(() => this.update(), 3000);
    }

    stop() {
        if (this.updateInterval) {
            clearInterval(this.updateInterval);
            this.updateInterval = null;
        }
    }
}

async function refreshCache() {
    if (window.cacheManager) {
        await window.cacheManager.update();
    }
}

async function clearCache() {
    if (confirm('确定要清空所有缓存吗？')) {
        alert('清空缓存功能需要后端 API 支持');
        // TODO: 实现清空缓存 API
    }
}

async function clearRecipientCache(recipient) {
    if (confirm(`确定要清除 ${recipient} 的缓存吗？`)) {
        alert('清除指定缓存功能需要后端 API 支持');
        // TODO: 实现清除指定缓存 API
    }
}
