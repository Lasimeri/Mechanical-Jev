# Entry points; `make help` lists them.

.DEFAULT_GOAL := help
MJEV := target/release/mjev
PREFIX ?= $(HOME)/.local

.PHONY: help build setup doctor install uninstall tui query eval models reconstruct evidence tokenizers closeness serve stop test fmt clippy docs-check tool-check check clean

help: ## Show this help
	@grep -E '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-14s %s\n", $$1, $$2}'

build: ## Build mjev
	cargo build --release

setup: build ## The whole family in one step: build and link mjev and xks, build what is missing, check down to the cards
	$(MJEV) doctor --fix --prefix "$(PREFIX)"

doctor: build ## What is missing for mjev to ask, and the fix for each (FIX=1 builds and links what it can)
	$(MJEV) doctor --prefix "$(PREFIX)" $(if $(FIX),--fix)

install: build ## Link mjev into $(PREFIX)/bin (a link: every rebuild is what runs; never over a file)
	@mkdir -p "$(PREFIX)/bin"
	@if [ -e "$(PREFIX)/bin/mjev" ] && [ ! -L "$(PREFIX)/bin/mjev" ]; then echo "$(PREFIX)/bin/mjev is a file, not a link: left alone"; exit 1; fi
	@ln -sfn "$(CURDIR)/$(MJEV)" "$(PREFIX)/bin/mjev" && echo "$(PREFIX)/bin/mjev -> $(CURDIR)/$(MJEV)"

uninstall: ## Remove that link (only a link, never a file)
	@if [ -L "$(PREFIX)/bin/mjev" ]; then rm "$(PREFIX)/bin/mjev" && echo "removed $(PREFIX)/bin/mjev"; else echo "no link at $(PREFIX)/bin/mjev"; fi

tui: build ## The terminal interface (FILE=request.json to open one)
	$(MJEV) tui $(if $(FILE),--file $(FILE))

query: build ## Ask examples/query.json (starts Intel Phi Jev if it is down)
	$(MJEV) query --file examples/query.json

eval: build ## Score the long real sessions; rows in target/eval-rows.jsonl
	$(MJEV) eval examples/long_sessions.jsonl --rows target/eval-rows.jsonl

models: build ## What the server serves
	$(MJEV) models

reconstruct: build ## What Jev most likely does with examples/query.json (offline)
	$(MJEV) reconstruct --file examples/query.json

closeness: build ## Ask Intel Phi Jev Jev's published questions and compare with Jev (LAYOUT, PERMUTATIONS)
	$(MJEV) evidence
	XKS_LAYOUT=$(or $(LAYOUT),letters) XKS_PERMUTATIONS=$(or $(PERMUTATIONS),3) $(MJEV) eval target/evidence-cases.jsonl --rows target/closeness-rows.jsonl
	$(MJEV) stop
	$(MJEV) corroborate target/evidence-jev-rows.jsonl target/closeness-rows.jsonl --temperature

evidence: ## The reverse-engineering fits, offline (the token-count fits need make tokenizers)
	cd tools/jevre && cargo run --release --offline -- probs
	@if [ -n "$$JEVRE_TOKENIZERS" ] || [ -d tools/jevre/tokenizers ]; then \
		cd tools/jevre && cargo run --release --offline -- input && cargo run --release --offline -- output; \
	else echo "evidence: input and output fits skipped, no tokenizers (make tokenizers)"; fi

tokenizers: ## Fetch the nine tokenizer.json files the fits read, pinned and sha256-checked (tools/jevre/TOKENIZERS)
	@grep -v '^#' tools/jevre/TOKENIZERS | while read -r name repo rev sha; do \
		f=tools/jevre/tokenizers/$$name/tokenizer.json; \
		if [ -f "$$f" ] && echo "$$sha  $$f" | sha256sum -c --status; then echo "$$name: present"; continue; fi; \
		hf download "$$repo" tokenizer.json --revision "$$rev" --local-dir "tools/jevre/tokenizers/$$name" >/dev/null || exit 1; \
		echo "$$sha  $$f" | sha256sum -c --quiet || exit 1; \
		echo "$$name: fetched"; \
	done

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

tool-check: ## Format and lint tools/jevre (offline)
	cd tools/jevre && cargo fmt --check && cargo clippy --release --offline -- -D warnings

check: docs-check fmt clippy build test tool-check ## Everything before a commit

clean: ## Remove build outputs
	cargo clean
