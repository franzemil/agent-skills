# agent-skills — cross-harness skill/subagent installer
SHELL := /bin/bash
BIN   := target/release/agent-skills
# rustup installs cargo in ~/.cargo/bin which is not always on non-login PATHs
export PATH := $(HOME)/.cargo/bin:$(PATH)

.PHONY: build install install-pi install-claude install-opencode install-all \
        sync list validate test clean

build: ## Build the release binary
	cargo build --release

install: build ## Smart wizard: detect folder, pick harness, items, Local/Global scope
	$(BIN)

install-pi: build ## Install into pi (TUI)
	$(BIN) --harness pi

install-claude: build ## Install into Claude Code (TUI)
	$(BIN) --harness claude

install-opencode: build ## Install into opencode (TUI)
	$(BIN) --harness opencode

install-all: build ## Install selection into every supported harness (TUI)
	$(BIN) --harness all

sync: build ## Re-apply last selection per harness from .installer-state.json
	$(BIN) --sync

list: build ## List all discovered skills + subagents (no TUI)
	$(BIN) --list

validate: build ## Validate frontmatter of all skills + subagents
	$(BIN) --validate

test: ## Run unit tests
	cargo test

clean: ## Remove build artifacts
	cargo clean

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}'

.DEFAULT_GOAL := help
