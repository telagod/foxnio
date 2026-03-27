.PHONY: build run test clean docker deploy dev help

# 默认目标
.DEFAULT_GOAL := help

# 颜色定义
CYAN := \033[36m
GREEN := \033[32m
YELLOW := \033[33m
NC := \033[0m

## 开发命令

dev: ## 启动开发环境
	@echo "$(CYAN)启动开发环境...$(NC)"
	./dev.sh

run: ## 运行后端服务
	@echo "$(CYAN)运行后端服务...$(NC)"
	cd backend && cargo run

run-frontend: ## 运行前端服务
	@echo "$(CYAN)运行前端服务...$(NC)"
	cd frontend && npm run dev

## 构建命令

build: ## 构建所有服务
	@echo "$(CYAN)构建后端...$(NC)"
	cd backend && cargo build --release
	@echo "$(CYAN)构建前端...$(NC)"
	cd frontend && npm run build

build-backend: ## 只构建后端
	@echo "$(CYAN)构建后端...$(NC)"
	cd backend && cargo build --release

build-frontend: ## 只构建前端
	@echo "$(CYAN)构建前端...$(NC)"
	cd frontend && npm run build

## 测试命令

test: ## 运行所有测试
	@echo "$(CYAN)运行测试...$(NC)"
	cd backend && cargo test
	cd frontend && npm test

test-backend: ## 只运行后端测试
	@echo "$(CYAN)运行后端测试...$(NC)"
	cd backend && cargo test

test-frontend: ## 只运行前端测试
	@echo "$(CYAN)运行前端测试...$(NC)"
	cd frontend && npm test

test-integration: ## 运行集成测试
	@echo "$(CYAN)运行集成测试...$(NC)"
	cd backend && cargo test --test integration_test

test-e2e: ## 运行端到端测试
	@echo "$(CYAN)运行端到端测试...$(NC)"
	cd backend && cargo test --test full_e2e_test

test-load: ## 运行负载测试
	@echo "$(CYAN)运行负载测试...$(NC)"
	cd backend && cargo test --test load_test -- --ignored

test-coverage: ## 运行测试并生成覆盖率报告
	@echo "$(CYAN)生成后端覆盖率报告...$(NC)"
	cd backend && cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info
	@echo "$(CYAN)生成前端覆盖率报告...$(NC)"
	cd frontend && npm run test:coverage
	@echo "$(GREEN)覆盖率报告已生成$(NC)"
	@echo "后端: backend/lcov.info"
	@echo "前端: frontend/coverage/lcov.info"

test-coverage-open: ## 打开覆盖率报告
	@echo "$(CYAN)打开后端覆盖率报告...$(NC)"
	cd backend && cargo llvm-cov --open
	@echo "$(CYAN)打开前端覆盖率报告...$(NC)"
	cd frontend && npm run test:coverage -- --reporter=html
	open frontend/coverage/index.html 2>/dev/null || xdg-open frontend/coverage/index.html 2>/dev/null || echo "请手动打开 frontend/coverage/index.html"

## Docker 命令

docker-build: ## 构建 Docker 镜像
	@echo "$(CYAN)构建 Docker 镜像...$(NC)"
	docker-compose build

docker-up: ## 启动 Docker 服务
	@echo "$(CYAN)启动 Docker 服务...$(NC)"
	docker-compose up -d

docker-down: ## 停止 Docker 服务
	@echo "$(CYAN)停止 Docker 服务...$(NC)"
	docker-compose down

docker-logs: ## 查看 Docker 日志
	docker-compose logs -f

docker-ps: ## 查看 Docker 容器状态
	docker-compose ps

## 数据库命令

db-migrate: ## 运行数据库迁移
	@echo "$(CYAN)运行迁移...$(NC)"
	cd backend && cargo sqlx migrate run

db-rollback: ## 回滚数据库迁移
	@echo "$(CYAN)回滚迁移...$(NC)"
	cd backend && cargo sqlx migrate revert

db-reset: ## 重置数据库
	@echo "$(CYAN)重置数据库...$(NC)"
	cd backend && cargo sqlx database reset

db-backup: ## 备份数据库
	@echo "$(CYAN)备份数据库...$(NC)"
	./deploy.sh backup

db-restore: ## 恢复数据库 (需要指定文件)
	@echo "$(CYAN)恢复数据库...$(NC)"
	./deploy.sh restore $(file)

## 部署命令

deploy: ## 部署服务
	@echo "$(CYAN)部署服务...$(NC)"
	./deploy.sh start

deploy-stop: ## 停止服务
	@echo "$(CYAN)停止服务...$(NC)"
	./deploy.sh stop

deploy-restart: ## 重启服务
	@echo "$(CYAN)重启服务...$(NC)"
	./deploy.sh restart

deploy-update: ## 更新服务
	@echo "$(CYAN)更新服务...$(NC)"
	./deploy.sh update

## 代码质量

lint: ## 运行代码检查
	@echo "$(CYAN)运行 Clippy...$(NC)"
	cd backend && cargo clippy -- -D warnings
	cd frontend && npm run lint

fmt: ## 格式化代码
	@echo "$(CYAN)格式化代码...$(NC)"
	cd backend && cargo fmt
	cd frontend && npm run format

check: ## 检查代码
	@echo "$(CYAN)检查代码...$(NC)"
	cd backend && cargo check
	cd backend && cargo clippy

## 清理命令

clean: ## 清理构建产物
	@echo "$(CYAN)清理...$(NC)"
	cd backend && cargo clean
	cd frontend && rm -rf build node_modules
	rm -rf target

clean-docker: ## 清理 Docker 资源
	@echo "$(CYAN)清理 Docker...$(NC)"
	docker-compose down -v
	docker system prune -af

## 工具命令

install: ## 安装依赖
	@echo "$(CYAN)安装依赖...$(NC)"
	cd backend && cargo fetch
	cd frontend && npm install

update: ## 更新依赖
	@echo "$(CYAN)更新依赖...$(NC)"
	cd backend && cargo update
	cd frontend && npm update

env: ## 创建环境配置
	@echo "$(CYAN)创建环境配置...$(NC)"
	cp .env.example .env
	@echo "$(GREEN)请编辑 .env 文件$(NC)"

logs: ## 查看服务日志
	docker-compose logs -f backend

status: ## 查看服务状态
	@echo "$(CYAN)服务状态:$(NC)"
	docker-compose ps
	@echo ""
	@echo "$(CYAN)健康检查:$(NC)"
	curl -s http://localhost:3000/health | jq . || echo "服务未启动"

## 帮助

help: ## 显示帮助信息
	@echo "$(CYAN)FoxNIO Makefile$(NC)"
	@echo ""
	@echo "$(YELLOW)使用方法:$(NC)"
	@echo "  make [target]"
	@echo ""
	@echo "$(YELLOW)可用命令:$(NC)"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(GREEN)%-15s$(NC) %s\n", $$1, $$2}'
