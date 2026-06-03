// 图表绘制模块
class ChartManager {
    constructor() {
        this.charts = {};
        this.updateInterval = null;
    }

    init(performanceMonitor) {
        this.performanceMonitor = performanceMonitor;
        this.createRelayRateChart();
        this.createCacheUsageChart();
        this.createLatencyChart();
        this.createThroughputChart();
        this.startAutoUpdate();
    }

    createRelayRateChart() {
        const ctx = document.getElementById('relayRateChart');
        if (!ctx) return;

        this.charts.relayRate = new Chart(ctx, {
            type: 'line',
            data: {
                labels: [],
                datasets: [{
                    label: '中转速率 (msg/s)',
                    data: [],
                    borderColor: 'rgb(37, 99, 235)',
                    backgroundColor: 'rgba(37, 99, 235, 0.1)',
                    tension: 0.4,
                    fill: true
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: true,
                plugins: {
                    legend: {
                        display: true,
                        position: 'top'
                    }
                },
                scales: {
                    y: {
                        beginAtZero: true
                    }
                }
            }
        });
    }

    createCacheUsageChart() {
        const ctx = document.getElementById('cacheUsageChart');
        if (!ctx) return;

        this.charts.cacheUsage = new Chart(ctx, {
            type: 'doughnut',
            data: {
                labels: ['已使用', '可用'],
                datasets: [{
                    data: [0, 100],
                    backgroundColor: [
                        'rgb(37, 99, 235)',
                        'rgb(226, 232, 240)'
                    ],
                    borderWidth: 0
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: true,
                plugins: {
                    legend: {
                        display: true,
                        position: 'bottom'
                    }
                }
            }
        });
    }

    createLatencyChart() {
        const ctx = document.getElementById('latencyChart');
        if (!ctx) return;

        this.charts.latency = new Chart(ctx, {
            type: 'line',
            data: {
                labels: [],
                datasets: [{
                    label: '延迟 (ms)',
                    data: [],
                    borderColor: 'rgb(16, 185, 129)',
                    backgroundColor: 'rgba(16, 185, 129, 0.1)',
                    tension: 0.4,
                    fill: true
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: true,
                plugins: {
                    legend: {
                        display: true,
                        position: 'top'
                    }
                },
                scales: {
                    y: {
                        beginAtZero: true
                    }
                }
            }
        });
    }

    createThroughputChart() {
        const ctx = document.getElementById('throughputChart');
        if (!ctx) return;

        this.charts.throughput = new Chart(ctx, {
            type: 'line',
            data: {
                labels: [],
                datasets: [
                    {
                        label: '中转总数',
                        data: [],
                        borderColor: 'rgb(37, 99, 235)',
                        backgroundColor: 'rgba(37, 99, 235, 0.1)',
                        tension: 0.4,
                        fill: true
                    },
                    {
                        label: '投递总数',
                        data: [],
                        borderColor: 'rgb(16, 185, 129)',
                        backgroundColor: 'rgba(16, 185, 129, 0.1)',
                        tension: 0.4,
                        fill: true
                    }
                ]
            },
            options: {
                responsive: true,
                maintainAspectRatio: true,
                plugins: {
                    legend: {
                        display: true,
                        position: 'top'
                    }
                },
                scales: {
                    y: {
                        beginAtZero: true
                    }
                }
            }
        });
    }

    async update() {
        try {
            const stats = await api.getStats();
            const cacheData = await api.getCache();

            this.updateRelayRateChart(stats);
            this.updateCacheUsageChart(stats, cacheData.stats);
            this.updateLatencyChart();
            this.updateThroughputChart();
        } catch (error) {
            console.error('Chart update failed:', error);
        }
    }

    updateRelayRateChart(stats) {
        const chart = this.charts.relayRate;
        if (!chart) return;

        const now = new Date().toLocaleTimeString('zh-CN', {
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit'
        });

        chart.data.labels.push(now);
        chart.data.datasets[0].data.push(stats.total_relayed || 0);

        if (chart.data.labels.length > 30) {
            chart.data.labels.shift();
            chart.data.datasets[0].data.shift();
        }

        chart.update('none');
    }

    updateCacheUsageChart(stats, cacheStats) {
        const chart = this.charts.cacheUsage;
        if (!chart) return;

        const used = cacheStats?.total_entries || 0;
        const max = cacheStats?.max_entries || 10000;
        const available = Math.max(0, max - used);

        chart.data.datasets[0].data = [used, available];
        chart.update('none');
    }

    updateLatencyChart() {
        const chart = this.charts.latency;
        if (!chart || !this.performanceMonitor) return;

        const data = this.performanceMonitor.getLatencyData();
        if (data.length === 0) return;

        chart.data.labels = data.map(d =>
            new Date(d.time).toLocaleTimeString('zh-CN', {
                hour: '2-digit',
                minute: '2-digit',
                second: '2-digit'
            })
        );
        chart.data.datasets[0].data = data.map(d => d.value);
        chart.update('none');
    }

    updateThroughputChart() {
        const chart = this.charts.throughput;
        if (!chart || !this.performanceMonitor) return;

        const data = this.performanceMonitor.getThroughputData();
        if (data.length === 0) return;

        chart.data.labels = data.map(d =>
            new Date(d.time).toLocaleTimeString('zh-CN', {
                hour: '2-digit',
                minute: '2-digit',
                second: '2-digit'
            })
        );
        chart.data.datasets[0].data = data.map(d => d.relayed);
        chart.data.datasets[1].data = data.map(d => d.delivered);
        chart.update('none');
    }

    startAutoUpdate() {
        this.updateInterval = setInterval(() => this.update(), 2000);
    }

    stop() {
        if (this.updateInterval) {
            clearInterval(this.updateInterval);
            this.updateInterval = null;
        }
        Object.values(this.charts).forEach(chart => chart.destroy());
        this.charts = {};
    }
}
