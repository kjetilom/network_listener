# The name of the binary
BINARY_NAME := network_listener

# Default target
all: build

# Build the project
build:
	cargo build --release

build_debug:
	cargo build

# Run the project
run: build
	sudo ./target/release/$(BINARY_NAME)

run_debug: build_debug
	sudo ./target/debug/$(BINARY_NAME)

test: build_debug
	cargo test -p network_listener --lib

# Clean the project
clean:
	cargo clean

.PHONY: all build run clean runbin
