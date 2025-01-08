# Makefile for a Rust project

# The name of the binary
BINARY_NAME := network_listener

MININET_WIFI := mnw.py

# The directory containing the source code
SRC_DIR := src

# The directory for build artifacts
TARGET_DIR := target

# Default target
all: build

# Build the project
build:
	cargo build --release

# Run the project
run: build
	sudo ./target/release/$(BINARY_NAME)

runbin:
	sudo ./target/release/$(BINARY_NAME)

mnw:
	sudo -E python $(MININET_WIFI)

# Clean the project
clean:
	cargo clean

.PHONY: all build run clean runbin
