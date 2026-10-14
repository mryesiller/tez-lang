# Tez developer commands. `make help` lists them.
# Rust comes from rustup (pinned by rust-toolchain.toml); Homebrew's plain `rust` has no wasm32 target.
RUSTUP_BIN := $(shell brew --prefix rustup 2>/dev/null)/bin
export PATH := $(RUSTUP_BIN):$(HOME)/.cargo/bin:$(PATH)

.PHONY: help check fmt fmt-check lint test wasm studio-install studio-dev studio-build doctor link-agents

help: ## List commands
	@grep -E '^[a-z-]+:.*##' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  %-15s %s\n", $$1, $$2}'

check: fmt-check lint test wasm studio-build ## Full local gate (same as CI)

fmt: ## Format Rust code
	cargo fmt --all

fmt-check: ## Verify formatting
	cargo fmt --all -- --check

lint: ## Clippy, warnings are errors
	cargo clippy --workspace --all-targets -- -D warnings

test: ## Run Rust tests
	cargo test --workspace

wasm: ## Build the portable crates for wasm32
	cargo build --target wasm32-unknown-unknown -p tez_diag -p tez_syntax -p tez_types

studio-install: ## Install Studio dependencies
	cd studio && pnpm install --frozen-lockfile

studio-dev: ## Run Studio dev server (COOP/COEP enabled)
	cd studio && pnpm dev

studio-build: studio-install ## Type-check and build Studio
	cd studio && pnpm build

doctor: ## Show toolchain versions
	@rustc --version; cargo --version; rustup target list --installed | grep wasm32 || echo "MISSING wasm32 target"
	@node --version; pnpm --version; wasm-tools --version; wasm-opt --version; z3 --version

link-agents: ## Symlink .agents/ into tool-specific locations
	.agents/link.sh
