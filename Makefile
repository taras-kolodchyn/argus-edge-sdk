COMPOSE_DIR := deploy/compose
COMPOSE_FILE := docker-compose.dev.yml
COMPOSE := cd $(COMPOSE_DIR) && docker compose -f $(COMPOSE_FILE)
CARGO_WORKSPACE := services/Cargo.toml

.PHONY: bootstrap dev-up dev-down dev-logs dev-build fmt lint test check workspace-clean

bootstrap:
	@cp -n $(COMPOSE_DIR)/.env.example $(COMPOSE_DIR)/.env 2>/dev/null || true
	@echo "Bootstrap complete (ensure $(COMPOSE_DIR)/.env is configured)."

dev-up: bootstrap
	@$(COMPOSE) up -d
	@echo "OTP tester loop active (inspect via 'make dev-logs SERVICE=otp-tester')."

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
	cargo fmt --all --manifest-path $(CARGO_WORKSPACE)

lint:
	cargo clippy --manifest-path $(CARGO_WORKSPACE) --workspace --all-targets --all-features -- -D warnings

check:
	cargo check --manifest-path $(CARGO_WORKSPACE) --workspace --all-targets --all-features

test:
	cargo test --manifest-path $(CARGO_WORKSPACE) --workspace --all-features -- --nocapture

workspace-clean:
	cargo clean --manifest-path $(CARGO_WORKSPACE)
