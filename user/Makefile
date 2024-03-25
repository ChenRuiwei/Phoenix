# Building variables
FEATURES :=
ifneq ($(PRELIMINARY), )
	FEATURES += preliminary
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

PHONY += build
build: $(APPS)
	cargo build $(CARGO_BUILD_ARGS)

.PHONY: build
