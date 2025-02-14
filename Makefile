# Makefile for a Rust project

# The name of the binary
BINARY_NAME := network_listener

POSTGRES_DB := pgrdb

SQL_SERVER := scheduler

# The directory containing the source code
SRC_DIR := src

# The directory for build artifacts
TARGET_DIR := target

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

runbin:
	sudo ./target/release/$(BINARY_NAME)

run_debug: build_debug
	sudo ./target/debug/$(BINARY_NAME)

run_debugbin:
	sudo ./target/debug/$(BINARY_NAME)

start-postgres:
	sudo docker run --name $(POSTGRES_DB) \
	  -e POSTGRES_USER=user \
	  -e POSTGRES_PASSWORD=password \
	  -e POSTGRES_DB=metricsdb \
	  -p 5432:5432 \
	  -d postgres:13

stop-postgres:
	-sudo docker stop $(POSTGRES_DB) || true
	-sudo docker rm $(POSTGRES_DB) || true

run_sch: build
	sudo ./target/release/$(SQL_SERVER) 172.16.0.254:50041

start-grafana:
	sudo docker run -d --name=grafana -p 3000:3000 grafana/grafana

stop-grafana:
	-sudo docker stop grafana || true
	-sudo docker rm grafana || true

# Clean the project
clean:
	cargo clean

.PHONY: all build run clean runbin
