BINDIR ?= $(HOME)/.local/bin

.PHONY: build install uninstall test clippy

build:
	cargo build --release

install: build
	install -Dm755 target/release/vois $(BINDIR)/vois
	@echo "installed: $(BINDIR)/vois"
	@echo "make sure $(BINDIR) is on your PATH, then just run: vois"

uninstall:
	rm -f $(BINDIR)/vois

test:
	cargo test --bin vois

clippy:
	cargo clippy --all-targets -- -D warnings
