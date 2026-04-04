#!/bin/bash
# FoxNIO 部署脚本

set -euo pipefail

COMPOSE_CMD=(docker compose)
CORE_SERVICES=(postgres redis backend)
UI_PROFILE=(--profile ui)
EDGE_PROFILE=(--profile ui --profile edge)

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

compose() {
    "${COMPOSE_CMD[@]}" "$@"
}

check_dependencies() {
    log_info "Checking dependencies..."

    command -v docker >/dev/null 2>&1 || { log_error "Docker is required but not installed."; exit 1; }
    docker compose version >/dev/null 2>&1 || { log_error "Docker Compose plugin is required."; exit 1; }

    log_info "Dependencies ready."
}

setup_env() {
    if [ -f .env ]; then
        log_warn ".env already exists. Skipping creation."
        return
    fi

    if [ ! -f .env.example ]; then
        log_error ".env.example not found."
        exit 1
    fi

    log_info "Creating .env from .env.example..."
    cp .env.example .env

    local jwt_secret
    jwt_secret=$(openssl rand -hex 32)
    sed -i "s|^JWT_SECRET=.*$|JWT_SECRET=${jwt_secret}|" .env

    local master_key
    master_key=$(openssl rand -hex 32)
    sed -i "s|^FOXNIO_MASTER_KEY=.*$|FOXNIO_MASTER_KEY=${master_key}|" .env

    log_info ".env created. Adjust values before production rollout if needed."
}

setup_ssl() {
    mkdir -p ssl

    if [ -f ssl/cert.pem ] && [ -f ssl/key.pem ]; then
        return
    fi

    log_warn "SSL certificates not found. Generating self-signed certificates for edge profile."
    openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
        -keyout ssl/key.pem \
        -out ssl/cert.pem \
        -subj "/CN=localhost"
}

setup_backup() {
    mkdir -p /var/backups/foxnio
    chmod 750 /var/backups/foxnio
}

wait_for_backend() {
    local retries=30

    log_info "Waiting for backend health..."
    until curl -fsS http://localhost:8080/health >/dev/null 2>&1; do
        retries=$((retries - 1))
        if [ "$retries" -le 0 ]; then
            log_error "Backend health check failed."
            compose logs backend
            exit 1
        fi
        sleep 2
    done

    log_info "Backend is healthy."
}

build_core() {
    log_info "Building core images..."
    compose build "${CORE_SERVICES[@]}"
}

build_ui() {
    log_info "Building UI images..."
    compose "${UI_PROFILE[@]}" build frontend
}

build_edge() {
    log_info "Building edge images..."
    setup_ssl
    compose "${EDGE_PROFILE[@]}" build frontend nginx
}

start_core() {
    log_info "Starting core services..."
    compose up -d "${CORE_SERVICES[@]}"
    wait_for_backend
}

start_ui() {
    log_info "Starting frontend profile..."
    compose "${UI_PROFILE[@]}" up -d frontend
}

start_edge() {
    log_info "Starting edge profile..."
    setup_ssl
    compose "${EDGE_PROFILE[@]}" up -d frontend nginx
}

stop_all() {
    log_info "Stopping all services..."
    compose down
}

logs() {
    compose logs -f "$@"
}

backup() {
    setup_backup

    local backup_file
    backup_file="/var/backups/foxnio/backup_$(date +%Y%m%d_%H%M%S).sql"

    log_info "Backing up database to ${backup_file}.gz ..."
    compose exec -T postgres pg_dump -U foxnio foxnio > "$backup_file"
    gzip "$backup_file"
    find /var/backups/foxnio -name "*.sql.gz" -mtime +30 -delete
}

restore() {
    if [ -z "${1:-}" ]; then
        log_error "Please provide backup file path."
        exit 1
    fi

    log_info "Restoring database from: $1"
    gunzip -c "$1" | compose exec -T postgres psql -U foxnio foxnio
}

update() {
    log_info "Updating repository..."
    git pull --ff-only
    build_core
    start_core
}

clean() {
    log_info "Cleaning up Docker resources..."
    compose down -v
    docker system prune -af
}

help() {
    cat <<'EOF'
Usage: ./deploy.sh <command>

Commands:
  build        Build postgres/redis/backend dependent images
  build-ui     Build frontend image
  build-edge   Build frontend + nginx edge image set
  start        Start core stack: postgres, redis, backend
  start-ui     Start frontend profile
  start-edge   Start nginx edge profile (also starts frontend)
  stop         Stop all services
  restart      Restart core stack
  logs         View logs (optional: service name)
  backup       Backup database
  restore      Restore database (requires file path)
  update       Pull latest code, rebuild, restart core stack
  clean        Remove compose resources and prune Docker
EOF
}

main() {
    case "${1:-}" in
        build)
            check_dependencies
            setup_env
            build_core
            ;;
        build-ui)
            check_dependencies
            setup_env
            build_ui
            ;;
        build-edge)
            check_dependencies
            setup_env
            build_edge
            ;;
        start)
            check_dependencies
            setup_env
            setup_backup
            start_core
            ;;
        start-ui)
            check_dependencies
            setup_env
            start_ui
            ;;
        start-edge)
            check_dependencies
            setup_env
            start_edge
            ;;
        stop)
            stop_all
            ;;
        restart)
            stop_all
            check_dependencies
            setup_env
            setup_backup
            start_core
            ;;
        logs)
            logs "${@:2}"
            ;;
        backup)
            backup
            ;;
        restore)
            restore "${2:-}"
            ;;
        update)
            check_dependencies
            setup_env
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
}

main "$@"
