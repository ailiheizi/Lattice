// 主应用逻辑
let dashboard;
let performanceMonitor;
let connectionManager;
let cacheManager;
let chartManager;
let currentTab = 'dashboard';

// 初始化应用
async function initApp() {
    // 检查连接状态
    await checkConnection();

    // 初始化各模块
    dashboard = new Dashboard(api);
    performanceMonitor = new PerformanceMonitor(api);
    connectionManager = new ConnectionManager(api);
    cacheManager = new CacheManager(api);
    chartManager = new ChartManager();

    // 初始化标签页
    initTabs();

    // 加载配置页面
    await loadConfig();

    // 启动仪表板
    await dashboard.init();

    // 初始化图表
    chartManager.init(performanceMonitor);

    // 定期检查连接
    setInterval(checkConnection, 5000);
}

// 检查连接状态
async function checkConnection() {
    const statusEl = document.getElementById('connectionStatus');
    if (!statusEl) return;

    try {
        const isHealthy = await api.health();
        if (isHealthy) {
            statusEl.className = 'status-indicator connected';
            statusEl.querySelector('.text').textContent = '已连接';
        } else {
            statusEl.className = 'status-indicator disconnected';
            statusEl.querySelector('.text').textContent = '连接失败';
        }
    } catch (error) {
        statusEl.className = 'status-indicator disconnected';
        statusEl.querySelector('.text').textContent = '未连接';
    }
}

// 初始化标签页
function initTabs() {
    const tabButtons = document.querySelectorAll('.tab-btn');
    tabButtons.forEach(btn => {
        btn.addEventListener('click', () => {
            const tabName = btn.dataset.tab;
            switchTab(tabName);
        });
    });
}

// 切换标签页
async function switchTab(tabName) {
    if (currentTab === tabName) return;

    // 停止当前标签页的更新
    stopCurrentTabUpdates();

    // 更新按钮状态
    document.querySelectorAll('.tab-btn').forEach(btn => {
        btn.classList.toggle('active', btn.dataset.tab === tabName);
    });

    // 更新内容显示
    document.querySelectorAll('.tab-content').forEach(content => {
        content.classList.toggle('active', content.id === tabName);
    });

    currentTab = tabName;

    // 启动新标签页的更新
    await startTabUpdates(tabName);
}

// 停止当前标签页的更新
function stopCurrentTabUpdates() {
    switch (currentTab) {
        case 'dashboard':
            dashboard?.stop();
            break;
        case 'performance':
            performanceMonitor?.stop();
            break;
        case 'connections':
            connectionManager?.stop();
            break;
        case 'cache':
            cacheManager?.stop();
            break;
    }
}

// 启动标签页更新
async function startTabUpdates(tabName) {
    switch (tabName) {
        case 'dashboard':
            await dashboard.init();
            break;
        case 'performance':
            await performanceMonitor.init();
            break;
        case 'connections':
            await connectionManager.init();
            connectionManager.startAutoUpdate();
            break;
        case 'cache':
            await cacheManager.init();
            cacheManager.startAutoUpdate();
            break;
    }
}

// 加载配置
async function loadConfig() {
    try {
        const config = await api.getConfig();
        document.getElementById('configListenAddr').value = config.listen_addr || '';
        document.getElementById('configMaxCache').value = config.max_cache_entries || '';
        document.getElementById('configTTL').value = config.cache_ttl_ms || '';
        document.getElementById('configEvictionInterval').value = config.eviction_interval_ms || '';
        document.getElementById('configProxyStores').value =
            (config.proxy_stores || []).join('\n');
    } catch (error) {
        console.error('Failed to load config:', error);
    }
}

// 日志相关功能
let logs = [];
let logLevel = 'info';

function refreshLogs() {
    // TODO: 实现日志获取 API
    console.log('Refresh logs');
}

function clearLogs() {
    logs = [];
    const viewer = document.getElementById('logViewer');
    if (viewer) {
        viewer.innerHTML = `
            <div class="log-entry log-info">
                <span class="log-time">--:--:--</span>
                <span class="log-level">INFO</span>
                <span class="log-message">日志已清空</span>
            </div>
        `;
    }
}

function filterLogs() {
    const select = document.getElementById('logLevel');
    if (select) {
        logLevel = select.value;
        // TODO: 实现日志过滤
    }
}

function addLogEntry(level, message) {
    const now = new Date();
    const time = now.toLocaleTimeString('zh-CN');

    logs.push({ time, level, message });

    const viewer = document.getElementById('logViewer');
    if (!viewer) return;

    const entry = document.createElement('div');
    entry.className = `log-entry log-${level}`;
    entry.innerHTML = `
        <span class="log-time">${time}</span>
        <span class="log-level">${level.toUpperCase()}</span>
        <span class="log-message">${message}</span>
    `;

    viewer.appendChild(entry);

    // 自动滚动
    const autoScroll = document.getElementById('autoScroll');
    if (autoScroll && autoScroll.checked) {
        viewer.scrollTop = viewer.scrollHeight;
    }

    // 限制日志数量
    if (logs.length > 1000) {
        logs.shift();
        viewer.removeChild(viewer.firstChild);
    }
}

// 页面加载完成后初始化
document.addEventListener('DOMContentLoaded', () => {
    initApp().catch(error => {
        console.error('App initialization failed:', error);
        addLogEntry('error', `应用初始化失败: ${error.message}`);
    });
});

// 页面卸载时清理
window.addEventListener('beforeunload', () => {
    dashboard?.stop();
    performanceMonitor?.stop();
    connectionManager?.stop();
    cacheManager?.stop();
    chartManager?.stop();
});
