#!/bin/bash

build-evn=SLINT_STYLE=fluent
run-evn=RUST_LOG=error,warn,info,debug,sqlx=off,reqwest=off

all:
	$(build-evn) cargo build --release

build:
	$(build-evn) cargo build --release

build-debug:
	$(build-evn) cargo build

run:
	$(build-evn) $(run-evn) cargo run

run-local-debug:
	$(run-evn) ./target/debug/bitbox

run-local-release:
	$(run-evn) ./target/release/bitbox

test:
	$(build-evn) $(run-evn) cargo test -- --nocapture

mold:
	$(build-evn) mold -run cargo build --release

mold-debug:
	$(build-evn) mold -run cargo build

clippy:
	cargo clippy

clean-incremental:
	rm -rf ./target/debug/incremental/*

clean:
	cargo clean

install:
	cp -rf ./target/release/bitbox ~/bin/

slint-view:
	slint-viewer --style fluent --auto-reload -I bitbox/ui ./bitbox/ui/appwindow.slint
