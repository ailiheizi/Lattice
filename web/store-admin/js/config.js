// Configuration Management
const CONFIG = {
    API_BASE_URL: 'http://localhost:9100',
    REFRESH_INTERVAL: 5000, // 5 seconds
    CHART_COLORS: {
        primary: '#3b82f6',
        success: '#10b981',
        warning: '#f59e0b',
        danger: '#ef4444',
        info: '#06b6d4'
    }
};

// API Endpoints
const API_ENDPOINTS = {
    STATS: '/stats',
    MESSAGES: '/messages',
    CONTACTS: '/contacts',
    ROOMS: '/rooms',
    LOGS: '/logs'
};

// Export for use in other modules
window.CONFIG = CONFIG;
window.API_ENDPOINTS = API_ENDPOINTS;
