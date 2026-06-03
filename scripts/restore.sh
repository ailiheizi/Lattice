#!/bin/bash
# NextIM Store 数据恢复脚本

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
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

log_prompt() {
    echo -e "${BLUE}[PROMPT]${NC} $1"
}

# 显示使用说明
usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Options:
  -b, --backup-dir DIR     Backup directory (default: /backup/nextim)
  -d, --data-dir DIR       Data directory (default: /var/lib/nextim/store)
  -f, --backup-file FILE   Specific backup file to restore
  -l, --list               List available backups
  -h, --help               Show this help message

Examples:
  $0 --list                                    # List all backups
  $0 --backup-file store_20260321_120000.db   # Restore specific backup
  $0                                           # Interactive restore

EOF
    exit 0
}

# 配置
BACKUP_DIR="${BACKUP_DIR:-/backup/nextim}"
DATA_DIR="${DATA_DIR:-/var/lib/nextim/store}"
BACKUP_FILE=""
LIST_ONLY=false

# 解析命令行参数
while [[ $# -gt 0 ]]; do
    case $1 in
        -b|--backup-dir)
            BACKUP_DIR="$2"
            shift 2
            ;;
        -d|--data-dir)
            DATA_DIR="$2"
            shift 2
            ;;
        -f|--backup-file)
            BACKUP_FILE="$2"
            shift 2
            ;;
        -l|--list)
            LIST_ONLY=true
            shift
            ;;
        -h|--help)
            usage
            ;;
        *)
            log_error "Unknown option: $1"
            usage
            ;;
    esac
done

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

# 列出可用的备份
list_backups() {
    log_info "Available backups in $BACKUP_DIR:"
    echo ""

    if [ ! -d "$BACKUP_DIR" ]; then
        log_error "Backup directory not found: $BACKUP_DIR"
        exit 1
    fi

    # 列出数据库备份
    echo "Database Backups:"
    echo "----------------"
    ls -lh "$BACKUP_DIR"/store_*.db 2>/dev/null | awk '{print $9, "(" $5 ")"}'

    echo ""
    echo "Search Index Backups:"
    echo "--------------------"
    ls -lh "$BACKUP_DIR"/search_*.tar.gz 2>/dev/null | awk '{print $9, "(" $5 ")"}'

    echo ""
    echo "Configuration Backups:"
    echo "---------------------"
    ls -lh "$BACKUP_DIR"/store_config_*.toml 2>/dev/null | awk '{print $9, "(" $5 ")"}'
}

# 选择备份文件
select_backup() {
    log_info "Selecting backup file..."

    # 获取所有数据库备份文件
    BACKUPS=($(ls -1t "$BACKUP_DIR"/store_*.db 2>/dev/null))

    if [ ${#BACKUPS[@]} -eq 0 ]; then
        log_error "No backup files found in $BACKUP_DIR"
        exit 1
    fi

    echo ""
    log_prompt "Available backups:"
    for i in "${!BACKUPS[@]}"; do
        BACKUP_NAME=$(basename "${BACKUPS[$i]}")
        BACKUP_SIZE=$(du -h "${BACKUPS[$i]}" | cut -f1)
        BACKUP_DATE=$(stat -c %y "${BACKUPS[$i]}" 2>/dev/null || stat -f %Sm "${BACKUPS[$i]}" 2>/dev/null)
        echo "  $((i+1)). $BACKUP_NAME ($BACKUP_SIZE) - $BACKUP_DATE"
    done

    echo ""
    read -p "Select backup number (1-${#BACKUPS[@]}): " SELECTION

    if [[ ! "$SELECTION" =~ ^[0-9]+$ ]] || [ "$SELECTION" -lt 1 ] || [ "$SELECTION" -gt ${#BACKUPS[@]} ]; then
        log_error "Invalid selection"
        exit 1
    fi

    BACKUP_FILE="${BACKUPS[$((SELECTION-1))]}"
    log_info "Selected backup: $(basename $BACKUP_FILE)"
}

# 停止服务
stop_service() {
    log_warn "Stopping NextIM Store service..."

    if command -v systemctl &> /dev/null; then
        if systemctl is-active --quiet nextim-store; then
            sudo systemctl stop nextim-store
            log_info "Service stopped"
        else
            log_info "Service is not running"
        fi
    else
        log_warn "systemctl not found, please stop the service manually"
        read -p "Press Enter when service is stopped..."
    fi
}

# 备份当前数据
backup_current_data() {
    log_info "Backing up current data before restore..."

    CURRENT_BACKUP_DIR="$DATA_DIR/backup_before_restore_$(date +%Y%m%d_%H%M%S)"
    mkdir -p "$CURRENT_BACKUP_DIR"

    if [ -f "$DATA_DIR/store.db" ]; then
        cp "$DATA_DIR/store.db" "$CURRENT_BACKUP_DIR/"
        log_info "Current database backed up to: $CURRENT_BACKUP_DIR"
    fi

    if [ -d "$DATA_DIR/search_index" ]; then
        cp -r "$DATA_DIR/search_index" "$CURRENT_BACKUP_DIR/"
        log_info "Current search index backed up to: $CURRENT_BACKUP_DIR"
    fi
}

# 恢复数据库
restore_database() {
    log_info "Restoring database..."

    if [ ! -f "$BACKUP_FILE" ]; then
        log_error "Backup file not found: $BACKUP_FILE"
        exit 1
    fi

    # 验证备份文件
    sqlite3 "$BACKUP_FILE" "PRAGMA integrity_check;" > /dev/null
    if [ $? -ne 0 ]; then
        log_error "Backup file is corrupted"
        exit 1
    fi

    # 恢复数据库
    cp "$BACKUP_FILE" "$DATA_DIR/store.db"

    if [ $? -eq 0 ]; then
        log_info "Database restored successfully"
    else
        log_error "Database restore failed"
        exit 1
    fi
}

# 恢复搜索索引
restore_search_index() {
    log_info "Restoring search index..."

    # 从备份文件名提取日期
    BACKUP_DATE=$(basename "$BACKUP_FILE" | sed 's/store_\(.*\)\.db/\1/')
    SEARCH_BACKUP="$BACKUP_DIR/search_${BACKUP_DATE}.tar.gz"

    if [ ! -f "$SEARCH_BACKUP" ]; then
        log_warn "Search index backup not found: $SEARCH_BACKUP"
        log_warn "Skipping search index restore"
        return
    fi

    # 删除旧的搜索索引
    if [ -d "$DATA_DIR/search_index" ]; then
        rm -rf "$DATA_DIR/search_index"
    fi

    # 恢复搜索索引
    tar -xzf "$SEARCH_BACKUP" -C "$DATA_DIR"

    if [ $? -eq 0 ]; then
        log_info "Search index restored successfully"
    else
        log_error "Search index restore failed"
        exit 1
    fi
}

# 恢复配置文件
restore_config() {
    log_info "Restoring configuration..."

    # 从备份文件名提取日期
    BACKUP_DATE=$(basename "$BACKUP_FILE" | sed 's/store_\(.*\)\.db/\1/')
    CONFIG_BACKUP="$BACKUP_DIR/store_config_${BACKUP_DATE}.toml"

    if [ ! -f "$CONFIG_BACKUP" ]; then
        log_warn "Configuration backup not found: $CONFIG_BACKUP"
        log_warn "Skipping configuration restore"
        return
    fi

    if [ -f "/etc/nextim/store.toml" ]; then
        log_warn "Configuration file already exists"
        read -p "Overwrite? (y/N): " OVERWRITE
        if [[ ! "$OVERWRITE" =~ ^[Yy]$ ]]; then
            log_info "Skipping configuration restore"
            return
        fi
    fi

    cp "$CONFIG_BACKUP" "/etc/nextim/store.toml"

    if [ $? -eq 0 ]; then
        log_info "Configuration restored successfully"
    else
        log_error "Configuration restore failed"
    fi
}

# 设置权限
set_permissions() {
    log_info "Setting permissions..."

    if command -v chown &> /dev/null; then
        sudo chown -R nextim:nextim "$DATA_DIR" 2>/dev/null || true
    fi

    chmod 600 "$DATA_DIR/store.db" 2>/dev/null || true
}

# 启动服务
start_service() {
    log_info "Starting NextIM Store service..."

    if command -v systemctl &> /dev/null; then
        sudo systemctl start nextim-store
        sleep 2
        if systemctl is-active --quiet nextim-store; then
            log_info "Service started successfully"
        else
            log_error "Service failed to start"
            exit 1
        fi
    else
        log_warn "systemctl not found, please start the service manually"
    fi
}

# 验证恢复
verify_restore() {
    log_info "Verifying restore..."

    # 验证数据库
    sqlite3 "$DATA_DIR/store.db" "PRAGMA integrity_check;" > /dev/null
    if [ $? -eq 0 ]; then
        log_info "Database verification passed"
    else
        log_error "Database verification failed"
        exit 1
    fi

    # 检查服务状态
    if command -v systemctl &> /dev/null; then
        if systemctl is-active --quiet nextim-store; then
            log_info "Service is running"
        else
            log_warn "Service is not running"
        fi
    fi
}

# 主函数
main() {
    log_info "NextIM Store Data Restore"
    log_info "Backup directory: $BACKUP_DIR"
    log_info "Data directory: $DATA_DIR"
    echo ""

    check_dependencies

    # 如果只是列出备份
    if [ "$LIST_ONLY" = true ]; then
        list_backups
        exit 0
    fi

    # 如果没有指定备份文件，交互式选择
    if [ -z "$BACKUP_FILE" ]; then
        select_backup
    else
        BACKUP_FILE="$BACKUP_DIR/$BACKUP_FILE"
    fi

    # 确认恢复
    echo ""
    log_warn "WARNING: This will replace current data with backup!"
    log_warn "Backup file: $(basename $BACKUP_FILE)"
    read -p "Continue? (y/N): " CONFIRM

    if [[ ! "$CONFIRM" =~ ^[Yy]$ ]]; then
        log_info "Restore cancelled"
        exit 0
    fi

    echo ""
    stop_service
    backup_current_data
    restore_database
    restore_search_index
    restore_config
    set_permissions
    start_service
    verify_restore

    echo ""
    log_info "Restore completed successfully!"
}

# 运行主函数
main "$@"
