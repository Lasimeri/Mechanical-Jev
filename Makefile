# Entry points; `make help` lists them.

.DEFAULT_GOAL := help
MJEV := target/release/mjev

.PHONY: help build query eval models serve stop test fmt clippy docs-check check clean

help: ## Show this help
	@grep -E '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-12s %s\n", $$1, $$2}'

build: ## Build mjev
	cargo build --release

query: build ## Ask examples/query.json (starts Intel Phi Jev if it is down)
	$(MJEV) query --file examples/query.json

eval: build ## Score the long real sessions; rows in target/eval-rows.jsonl
	$(MJEV) eval examples/long_sessions.jsonl --rows target/eval-rows.jsonl

models: build ## What the server serves
	$(MJEV) models

serve: build ## Start Intel Phi Jev's server
	$(MJEV) serve

stop: build ## Stop it and release the Phi cards
	$(MJEV) stop

test: ## Unit tests (no key needed)
	cargo test --release

fmt: ## Check formatting
	cargo fmt --all -- --check

clippy: ## Lint
	cargo clippy --release --all-targets -- -D warnings

docs-check: ## Sibling .md files, the no-dash rule, relative links
	scripts/check-docs.sh

check: docs-check fmt clippy build test ## Everything before a commit

clean: ## Remove build outputs
	cargo clean
