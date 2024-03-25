# Building variables
FEATURES := stack_trace
ifneq ($(STRACE), )
	FEATURES += strace
endif
ifneq ($(SUBMIT), )
	FEATURES += submit
endif
ifneq ($(TMPFS), )
	FEATURES += tmpfs
endif
ifneq ($(SMP), )
	FEATURES += multi_hart
endif
ifeq ($(BOARD), u740)
	FEATURES += board_u740
endif
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

PHONY := all
all: build

PHONY += build
build:
	@cp src/linker64.ld src/linker.ld
	cargo build $(CARGO_BUILD_ARGS)
	@rm src/linker.ld

.PHONY: $(PHONY)
