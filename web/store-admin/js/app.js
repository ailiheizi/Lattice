// Main Application
class App {
    constructor() {
        this.currentPage = 'dashboard';
        this.modules = {
            dashboard: window.dashboard,
            messages: window.messages,
            contacts: window.contacts,
            rooms: window.rooms,
            logs: window.logs
        };
    }

    async init() {
        this.setupNavigation();
        this.setupConnectionStatus();
        await this.switchPage('dashboard');
        this.checkConnection();
    }

    setupNavigation() {
        const navLinks = document.querySelectorAll('.nav-link');
        navLinks.forEach(link => {
            link.addEventListener('click', (e) => {
                e.preventDefault();
                const page = link.dataset.page;
                if (page) {
                    this.switchPage(page);
                }
            });
        });
    }

    async switchPage(pageName) {
        // Deactivate current module
        if (this.modules[this.currentPage]) {
            this.modules[this.currentPage].destroy();
        }

        // Hide all pages
        document.querySelectorAll('.page').forEach(page => {
            page.classList.remove('active');
        });

        // Remove active state from nav links
        document.querySelectorAll('.nav-link').forEach(link => {
            link.classList.remove('active');
        });

        // Show selected page
        const pageElement = document.getElementById(`${pageName}-page`);
        if (pageElement) {
            pageElement.classList.add('active');
        }

        // Activate nav link
        const navLink = document.querySelector(`.nav-link[data-page="${pageName}"]`);
        if (navLink) {
            navLink.classList.add('active');
        }

        // Initialize new module
        this.currentPage = pageName;
        if (this.modules[pageName]) {
            await this.modules[pageName].init();
        }
    }

    setupConnectionStatus() {
        this.connectionCheckInterval = setInterval(() => {
            this.checkConnection();
        }, 10000); // Check every 10 seconds
    }

    async checkConnection() {
        const statusIndicator = document.getElementById('connection-status');
        if (!statusIndicator) return;

        try {
            await api.getStats();
            statusIndicator.className = 'status-indicator status-online';
            statusIndicator.title = 'Connected to Store';
        } catch (error) {
            statusIndicator.className = 'status-indicator status-offline';
            statusIndicator.title = 'Disconnected from Store';
        }
    }
}

// Utility Functions
function showLoading(containerId) {
    const container = document.getElementById(containerId);
    if (container) {
        const loader = document.createElement('div');
        loader.className = 'loading-spinner';
        loader.innerHTML = '<div class="spinner"></div>';
        container.innerHTML = '';
        container.appendChild(loader);
    }
}

function hideLoading(containerId) {
    const container = document.getElementById(containerId);
    if (container) {
        const loader = container.querySelector('.loading-spinner');
        if (loader) {
            loader.remove();
        }
    }
}

function showError(containerId, message) {
    const container = document.getElementById(containerId);
    if (container) {
        container.innerHTML = `
            <div class="error-state">
                <svg width="48" height="48" fill="currentColor" viewBox="0 0 20 20">
                    <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clip-rule="evenodd"/>
                </svg>
                <p>${message}</p>
            </div>
        `;
    }
}

function showNotification(message, type = 'info') {
    const notification = document.createElement('div');
    notification.className = `notification notification-${type}`;
    notification.textContent = message;

    document.body.appendChild(notification);

    // Trigger animation
    setTimeout(() => notification.classList.add('show'), 10);

    // Remove after 3 seconds
    setTimeout(() => {
        notification.classList.remove('show');
        setTimeout(() => notification.remove(), 300);
    }, 3000);
}

// Initialize app when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
    window.app = new App();
    window.app.init();
});

// Export utility functions
window.showLoading = showLoading;
window.hideLoading = hideLoading;
window.showError = showError;
window.showNotification = showNotification;
