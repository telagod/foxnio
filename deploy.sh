#!/bin/bash
# FoxNIO 部署脚本

set -e

echo "🦊 FoxNIO Deployment Script"
echo "============================"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 日志函数
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
    
    command -v docker >/dev/null 2>&1 || { log_error "Docker is required but not installed."; exit 1; }
    command -v docker-compose >/dev/null 2>&1 || { log_error "Docker Compose is required but not installed."; exit 1; }
    
    log_info "All dependencies are installed."
}

# 创建环境文件
setup_env() {
    if [ ! -f .env ]; then
        log_info "Creating .env file from .env.example..."
        cp .env.example .env
        
        # 生成随机 JWT secret
        JWT_SECRET=$(openssl rand -hex 32)
        sed -i "s/your-super-secret-jwt-key-change-in-production/$JWT_SECRET/g" .env
        
        log_info "Environment file created. Please edit .env with your settings."
    else
        log_warn ".env file already exists. Skipping."
    fi
}

# 创建 SSL 目录
setup_ssl() {
    log_info "Setting up SSL directories..."
    mkdir -p ssl
    mkdir -p /var/www/certbot
    
    if [ ! -f ssl/cert.pem ]; then
        log_warn "SSL certificates not found. Generating self-signed certificates..."
        openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
            -keyout ssl/key.pem \
            -out ssl/cert.pem \
            -subj "/CN=localhost"
        log_info "Self-signed certificates generated."
    fi
}

# 创建备份目录
setup_backup() {
    log_info "Setting up backup directory..."
    mkdir -p /var/backups/foxnio
    chmod 750 /var/backups/foxnio
}

# 构建镜像
build() {
    log_info "Building Docker images..."
    docker-compose build
    log_info "Build completed."
}

# 启动服务
start() {
    log_info "Starting services..."
    docker-compose up -d
    log_info "Services started."
    
    log_info "Waiting for services to be ready..."
    sleep 10
    
    # 健康检查
    if curl -f http://localhost:3000/health >/dev/null 2>&1; then
        log_info "Backend is healthy."
    else
        log_error "Backend health check failed."
        exit 1
    fi
}

# 停止服务
stop() {
    log_info "Stopping services..."
    docker-compose down
    log_info "Services stopped."
}

# 重启服务
restart() {
    stop
    start
}

# 查看日志
logs() {
    docker-compose logs -f "$@"
}

# 备份数据库
backup() {
    log_info "Backing up database..."
    
    BACKUP_FILE="/var/backups/foxnio/backup_$(date +%Y%m%d_%H%M%S).sql"
    
    docker-compose exec -T postgres pg_dump -U foxnio foxnio > "$BACKUP_FILE"
    
    gzip "$BACKUP_FILE"
    
    log_info "Database backed up to: ${BACKUP_FILE}.gz"
    
    # 清理旧备份
    find /var/backups/foxnio -name "*.sql.gz" -mtime +30 -delete
    log_info "Old backups cleaned."
}

# 恢复数据库
restore() {
    if [ -z "$1" ]; then
        log_error "Please provide backup file path."
        exit 1
    fi
    
    log_info "Restoring database from: $1"
    
    gunzip -c "$1" | docker-compose exec -T postgres psql -U foxnio foxnio
    
    log_info "Database restored."
}

# 更新服务
update() {
    log_info "Updating services..."
    
    git pull
    
    docker-compose down
    docker-compose build
    docker-compose up -d
    
    log_info "Services updated."
}

# 清理
clean() {
    log_info "Cleaning up..."
    
    docker-compose down -v
    docker system prune -af
    
    log_info "Cleanup completed."
}

# 帮助信息
help() {
    echo "Usage: $0 {build|start|stop|restart|logs|backup|restore|update|clean}"
    echo ""
    echo "Commands:"
    echo "  build    - Build Docker images"
    echo "  start    - Start services"
    echo "  stop     - Stop services"
    echo "  restart  - Restart services"
    echo "  logs     - View logs (optional: service name)"
    echo "  backup   - Backup database"
    echo "  restore  - Restore database (requires file path)"
    echo "  update   - Update services"
    echo "  clean    - Clean up Docker resources"
}

# 主入口
case "$1" in
    build)
        check_dependencies
        build
        ;;
    start)
        check_dependencies
        setup_env
        setup_ssl
        setup_backup
        start
        ;;
    stop)
        stop
        ;;
    restart)
        restart
        ;;
    logs)
        logs "${@:2}"
        ;;
    backup)
        backup
        ;;
    restore)
        restore "$2"
        ;;
    update)
        update
        ;;
    clean)
        clean
        ;;
    *)
        help
        exit 1
        ;;
esac
