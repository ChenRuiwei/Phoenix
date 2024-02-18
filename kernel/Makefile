ifeq ($(MODE), release)
	MODE_ARG = --release
endif

PHONY := all
all: build

PHONY += build
build:
	@cargo build $(MODE_ARG)

.PHONY: $(PHONY)
