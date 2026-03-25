CARGO := cargo
BIN_NAME := lgtmcli
INSTALL_DIR ?= $(HOME)/.local/bin

.DEFAULT_GOAL := release

.PHONY: release build lint test install

# Default target (make): build release binary
release:
	$(CARGO) build --release

# make build: build debug binary
build:
	$(CARGO) build

# make lint: format, type-check, lint
lint:
	$(CARGO) fmt --all
	$(CARGO) check
	$(CARGO) clippy --all-targets --all-features -- -D warnings

# make test: run tests
test:
	$(CARGO) test

# make install: copy release binary to ~/.local/bin
install: release
	mkdir -p $(INSTALL_DIR)
	cp target/release/$(BIN_NAME) $(INSTALL_DIR)/$(BIN_NAME)
	chmod +x $(INSTALL_DIR)/$(BIN_NAME)
	@echo "Installed $(BIN_NAME) to $(INSTALL_DIR)/$(BIN_NAME)"
