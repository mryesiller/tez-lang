# Tez developer commands. `make help` lists them.
# Rust comes from rustup (pinned by rust-toolchain.toml); Homebrew's plain `rust` has no wasm32 target.
RUSTUP_BIN := $(shell brew --prefix rustup 2>/dev/null)/bin
export PATH := $(RUSTUP_BIN):$(HOME)/.cargo/bin:$(PATH)

.PHONY: agent-test bench-check gbnf site-dev site-build help check fmt fmt-check lint test wasm studio-wasm studio-install studio-dev studio-build doctor link-agents

help: ## List commands
	@grep -E '^[a-z-]+:.*##' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  %-15s %s\n", $$1, $$2}'

check: fmt-check lint test gbnf wasm studio-build ## Full local gate (same as CI)

bench-check: ## TezBench reference solutions must pass (builds the debug CLI first)
	cargo build -p tez-lang
	node bench/tezbench/run.mjs --self-check

agent-test: ## Node tests: repair loop, MCP server, data pipeline (needs the debug CLI)
	cargo build -p tez-lang
	node --test runtime/tez-app.test.mjs tools/agent/loop.test.mjs tools/tez-mcp/server.test.mjs data/data.test.mjs

gbnf: ## Verify spec/grammar.gbnf is current and every example is derivable
	python3 spec/tools/ebnf2gbnf.py --check

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

studio-wasm: ## Build the compiler for the browser into studio/src/tez-wasm
	cargo build --release --target wasm32-unknown-unknown -p tez_web
	rm -rf studio/src/tez-wasm
	wasm-bindgen --target web --out-dir studio/src/tez-wasm target/wasm32-unknown-unknown/release/tez_web.wasm
	wasm-opt -Oz --enable-bulk-memory --enable-sign-ext --enable-nontrapping-float-to-int --enable-mutable-globals -o studio/src/tez-wasm/tez_web_bg.opt.wasm studio/src/tez-wasm/tez_web_bg.wasm
	mv studio/src/tez-wasm/tez_web_bg.opt.wasm studio/src/tez-wasm/tez_web_bg.wasm

studio-install: ## Install Studio dependencies
	cd studio && pnpm install --frozen-lockfile

studio-dev: studio-wasm ## Run Studio dev server (COOP/COEP enabled)
	cd studio && pnpm dev

studio-build: studio-wasm studio-install ## Type-check and build Studio
	cd studio && pnpm build

site-dev: studio-wasm ## Run the website (landing + docs) dev server
	cd site && pnpm install && pnpm dev

site-build: studio-wasm ## Type-check and build the website into site/dist
	cd site && pnpm install --frozen-lockfile && pnpm build

doctor: ## Show toolchain versions
	@rustc --version; cargo --version; rustup target list --installed | grep wasm32 || echo "MISSING wasm32 target"
	@node --version; pnpm --version; wasm-tools --version; wasm-opt --version; z3 --version

link-agents: ## Symlink .agents/ into tool-specific locations
	.agents/link.sh
