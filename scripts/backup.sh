#!/bin/bash
# Lattice Store 数据备份脚本

set -e

# 配置
BACKUP_DIR="${BACKUP_DIR:-/backup/lattice}"
DATA_DIR="${DATA_DIR:-/var/lib/lattice/store}"
RETENTION_DAYS="${RETENTION_DAYS:-7}"
DATE=$(date +%Y%m%d_%H%M%S)

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检查依赖
check_dependencies() {
    log_info "Checking dependencies..."

    if ! command -v sqlite3 &> /dev/null; then
        log_error "sqlite3 is not installed"
        exit 1
    fi

    if ! command -v tar &> /dev/null; then
        log_error "tar is not installed"
        exit 1
    fi
}

# 创建备份目录
create_backup_dir() {
    log_info "Creating backup directory: $BACKUP_DIR"
    mkdir -p "$BACKUP_DIR"
}

# 备份数据库
backup_database() {
    log_info "Backing up database..."

    if [ ! -f "$DATA_DIR/store.db" ]; then
        log_error "Database file not found: $DATA_DIR/store.db"
        exit 1
    fi

    # 使用 SQLite 的 .backup 命令进行在线备份
    sqlite3 "$DATA_DIR/store.db" ".backup $BACKUP_DIR/store_$DATE.db"

    if [ $? -eq 0 ]; then
        log_info "Database backup completed: store_$DATE.db"
    else
        log_error "Database backup failed"
        exit 1
    fi
}

# 备份搜索索引
backup_search_index() {
    log_info "Backing up search index..."

    if [ ! -d "$DATA_DIR/search_index" ]; then
        log_warn "Search index directory not found, skipping..."
        return
    fi

    tar -czf "$BACKUP_DIR/search_$DATE.tar.gz" -C "$DATA_DIR" search_index/

    if [ $? -eq 0 ]; then
        log_info "Search index backup completed: search_$DATE.tar.gz"
    else
        log_error "Search index backup failed"
        exit 1
    fi
}

# 备份配置文件
backup_config() {
    log_info "Backing up configuration..."

    if [ -f "/etc/lattice/store.toml" ]; then
        cp "/etc/lattice/store.toml" "$BACKUP_DIR/store_config_$DATE.toml"
        log_info "Configuration backup completed: store_config_$DATE.toml"
    else
        log_warn "Configuration file not found, skipping..."
    fi
}

# 验证备份
verify_backup() {
    log_info "Verifying backup..."

    # 验证数据库备份
    if [ -f "$BACKUP_DIR/store_$DATE.db" ]; then
        sqlite3 "$BACKUP_DIR/store_$DATE.db" "PRAGMA integrity_check;" > /dev/null
        if [ $? -eq 0 ]; then
            log_info "Database backup verification passed"
        else
            log_error "Database backup verification failed"
            exit 1
        fi
    fi

    # 验证搜索索引备份
    if [ -f "$BACKUP_DIR/search_$DATE.tar.gz" ]; then
        tar -tzf "$BACKUP_DIR/search_$DATE.tar.gz" > /dev/null
        if [ $? -eq 0 ]; then
            log_info "Search index backup verification passed"
        else
            log_error "Search index backup verification failed"
            exit 1
        fi
    fi
}

# 清理旧备份
cleanup_old_backups() {
    log_info "Cleaning up old backups (older than $RETENTION_DAYS days)..."

    find "$BACKUP_DIR" -name "store_*.db" -mtime +$RETENTION_DAYS -delete
    find "$BACKUP_DIR" -name "search_*.tar.gz" -mtime +$RETENTION_DAYS -delete
    find "$BACKUP_DIR" -name "store_config_*.toml" -mtime +$RETENTION_DAYS -delete

    log_info "Old backups cleaned up"
}

# 生成备份报告
generate_report() {
    log_info "Generating backup report..."

    REPORT_FILE="$BACKUP_DIR/backup_report_$DATE.txt"

    cat > "$REPORT_FILE" <<EOF
Lattice Store Backup Report
==========================

Backup Date: $(date)
Backup Directory: $BACKUP_DIR

Files:
------
EOF

    if [ -f "$BACKUP_DIR/store_$DATE.db" ]; then
        DB_SIZE=$(du -h "$BACKUP_DIR/store_$DATE.db" | cut -f1)
        echo "- Database: store_$DATE.db ($DB_SIZE)" >> "$REPORT_FILE"
    fi

    if [ -f "$BACKUP_DIR/search_$DATE.tar.gz" ]; then
        SEARCH_SIZE=$(du -h "$BACKUP_DIR/search_$DATE.tar.gz" | cut -f1)
        echo "- Search Index: search_$DATE.tar.gz ($SEARCH_SIZE)" >> "$REPORT_FILE"
    fi

    if [ -f "$BACKUP_DIR/store_config_$DATE.toml" ]; then
        CONFIG_SIZE=$(du -h "$BACKUP_DIR/store_config_$DATE.toml" | cut -f1)
        echo "- Configuration: store_config_$DATE.toml ($CONFIG_SIZE)" >> "$REPORT_FILE"
    fi

    echo "" >> "$REPORT_FILE"
    echo "Total Backups: $(ls -1 $BACKUP_DIR/store_*.db 2>/dev/null | wc -l)" >> "$REPORT_FILE"
    echo "Disk Usage: $(du -sh $BACKUP_DIR | cut -f1)" >> "$REPORT_FILE"

    log_info "Backup report generated: $REPORT_FILE"
}

# 主函数
main() {
    log_info "Starting Lattice Store backup..."
    log_info "Backup directory: $BACKUP_DIR"
    log_info "Data directory: $DATA_DIR"
    log_info "Retention days: $RETENTION_DAYS"
    echo ""

    check_dependencies
    create_backup_dir
    backup_database
    backup_search_index
    backup_config
    verify_backup
    cleanup_old_backups
    generate_report

    echo ""
    log_info "Backup completed successfully!"
    log_info "Backup files:"
    ls -lh "$BACKUP_DIR"/*_$DATE.* 2>/dev/null || true
}

# 运行主函数
main "$@"
