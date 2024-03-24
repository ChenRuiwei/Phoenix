# Building variables
DOCKER_NAME = my-os
PACKAGE_NAME = kernel
TARGET = riscv64gc-unknown-none-elf
export BOARD = qemu
export MODE = release
export LOG = trace


# Tools
QEMU = qemu-system-riscv64
RISCV_GDB ?= riscv64-unknown-elf-gdb
OBJDUMP = rust-objdump --arch-name=riscv64
OBJCOPY = rust-objcopy --binary-architecture=riscv64
PAGER ?= less


# Target files
TARGET_DIR := target/$(TARGET)/$(MODE)
VENDOR_DIR := ./third-party/vendor
KERNEL_ELF := $(TARGET_DIR)/$(PACKAGE_NAME)
# be aware that make has implict rule on .S suffix
KERNEL_ASM := $(TARGET_DIR)/$(PACKAGE_NAME).asm


# Args
DOCKER_RUN_ARGS := run
DOCKER_RUN_ARGS += --rm
DOCKER_RUN_ARGS += -it
DOCKER_RUN_ARGS += --privileged
DOCKER_RUN_ARGS += --network="host"
DOCKER_RUN_ARGS += -v $(PWD):/mnt
DOCKER_RUN_ARGS += -v /dev:/dev
DOCKER_RUN_ARGS += -w /mnt
DOCKER_RUN_ARGS += $(DOCKER_NAME)
DOCKER_RUN_ARGS += bash

CPUS := 2
BOOTLOADER = default
QEMU_ARGS :=
QEMU_ARGS += -m 128M
QEMU_ARGS += -machine virt
QEMU_ARGS += -nographic
QEMU_ARGS += -smp $(CPUS)
QEMU_ARGS += -bios $(BOOTLOADER)
QEMU_ARGS += -kernel $(KERNEL_ELF)

GDB_ARGS := -ex 'file $(KERNEL_ELF)'
GDB_ARGS += -ex 'set arch riscv:rv64'
GDB_ARGS += -ex 'target remote localhost:1234'

DISASM_ARGS = -d

	
# Phony targets
PHONY := all
all: build run

PHONY += build_docker
build_docker:
	@docker build -t ${DOCKER_NAME} .

PHONY += docker
docker:
	@docker run --rm -it --network="host" -v ${PWD}:/mnt -w /mnt ${DOCKER_NAME} bash

PHONY += env
env:
	@(cargo install --list | grep "cargo-binutils" > /dev/null 2>&1) || cargo install cargo-binutils
	@cargo vendor $(VENDOR_DIR)

PHONY += build
build:
	@echo Platform: $(BOARD)
	@cd kernel && make build
	@echo "Updated: $(KERNEL_ELF)"

PHONY += run
run:
	@$(QEMU) $(QEMU_ARGS)

PHONY += clean
clean:
	@cargo clean
	@rm -rf $(TARGET_DIR)/*

PHONY += disasm
disasm:
	@$(PAGER) $(KERNEL_ASM)

PHONY += gdbserver
gdbserver:
	@$(QEMU) $(QEMU_ARGS) -s -S

PHONY += gdbclient
gdbclient:
	@$(RISCV_GDB) -ex 'file $(KERNEL_ELF)' -ex 'set arch riscv:rv64' -ex 'target remote localhost:1234'

.PHONY: $(PHONY)
