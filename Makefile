default: build

.PHONY: build
build:
	@cargo build --release

.PHONY: test
test:
	@cargo test

.PHONY: sanity
sanity:
	@cargo fmt --all --check
	@cargo clippy --all-targets --all-features -- -D warnings

.PHONY: format
format:
	@cargo fmt --all
