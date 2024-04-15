# Building variables
FEATURES :=
ifneq ($(STRACE), )
	FEATURES += strace
endif
ifneq ($(SMP), )
	FEATURES += smp
endif

CARGO_BUILD_ARGS :=
ifeq ($(MODE), release)
	 CARGO_BUILD_ARGS += --release
endif
ifneq ($(FEATURES), )
	CARGO_BUILD_ARGS += --features "$(FEATURES)"
endif

# Phony targets
PHONY :=

PHONY := all
all: build

PHONY += build
build:
	cargo build $(CARGO_BUILD_ARGS)

.PHONY: $(PHONY)
