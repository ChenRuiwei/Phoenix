ifeq ($(MODE), release)
	MODE_ARG = --release
endif

PHONY := all
all: build

PHONY += env
env:
	@(cargo install --list | grep "cargo-binutils" > /dev/null 2>&1) || cargo install cargo-binutils

PHONY += build
build: env
	@cargo build $(MODE_ARG)

.PHONY: $(PHONY)
