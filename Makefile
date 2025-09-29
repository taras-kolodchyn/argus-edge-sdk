COMPOSE_DIR := deploy/compose
COMPOSE_FILE := docker-compose.dev.yml
COMPOSE := cd $(COMPOSE_DIR) && docker compose -f $(COMPOSE_FILE)

.PHONY: bootstrap dev-up dev-down dev-logs dev-build fmt lint test check workspace-clean

bootstrap:
	@cp -n $(COMPOSE_DIR)/.env.example $(COMPOSE_DIR)/.env 2>/dev/null || true
	@echo "Bootstrap complete (ensure $(COMPOSE_DIR)/.env is configured)."

dev-up: bootstrap
	@$(COMPOSE) up -d

dev-down:
	@$(COMPOSE) down -v

dev-build:
	@$(COMPOSE) build mock-auth mock-sink mock-ota

dev-logs:
	@if [ -n "$(SERVICE)" ]; then \
		$(COMPOSE) logs -f $(SERVICE); \
	else \
		$(COMPOSE) logs -f; \
	fi

fmt:
	cargo fmt --all

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

check:
	cargo check --workspace --all-targets --all-features

test:
	cargo test --workspace --all-features -- --nocapture

workspace-clean:
	cargo clean
