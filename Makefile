all: help
help:
	@echo "targets:"
	@echo "- build: make a static binary"
	@echo "- image: build container image with docker"

build: target/x86_64-unknown-linux-musl/release/richard

target/x86_64-unknown-linux-musl/release/richard: src/*.rs
	cargo build --target x86_64-unknown-linux-musl --release

image: target/x86_64-unknown-linux-musl/release/richard
	docker build -t richard:latest .

.PHONY: cargo-test
cargo-test:	
	cargo test

.PHONY: format-test
format-test:
	cargo fmt --check
	cargo clippy

.PHONY: format
format:
	cargo fmt
