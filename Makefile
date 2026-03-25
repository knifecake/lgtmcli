CARGO := cargo

.DEFAULT_GOAL := release

.PHONY: release build lint test

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
