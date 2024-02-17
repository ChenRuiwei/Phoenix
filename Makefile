# Building variables
DOCKER_NAME = my-os
PACKAGE_NAME = kernel
BOOTLOADER = default
TARGET = riscv64gc-unknown-none-elf
export BOARD = qemu
export MODE = debug
export LOG = trace

# Tools
QEMU = qemu-system-riscv64
GDB = riscv64-elf-gdb
OBJDUMP = rust-objdump --arch-name=riscv64
OBJCOPY = rust-objcopy --binary-architecture=riscv64
PAGER ?= less

# Args
DISASM_ARGS = -d
QEMU_ARGS = -machine virt \
			 -nographic \
			 -bios $(BOOTLOADER) \
			 -kernel $(KERNEL_ELF)
	
# Target files
TARGET_DIR := target/$(TARGET)/$(MODE)
KERNEL_ELF := $(TARGET_DIR)/$(PACKAGE_NAME)
# be aware that make has implict rule on .S suffix
KERNEL_ASM := $(TARGET_DIR)/$(PACKAGE_NAME).asm

# Default target
PHONY := all
all: $(KERNEL_ELF) $(KERNEL_ASM)

# Target file dependencies
$(KERNEL_ELF): build

$(KERNEL_ASM): $(KERNEL_ELF)
	@$(OBJDUMP) $(DISASM_ARGS) $(KERNEL_ELF) > $(KERNEL_ASM)
	@echo "Updated: $(KERNEL_ASM)"

# Phony targets
PHONY += build_docker
build_docker:
	@docker build -t ${DOCKER_NAME} .

PHONY += docker
docker:
	@docker run --rm -it --network="host" -v ${PWD}:/mnt -w /mnt ${DOCKER_NAME} bash

PHONY += build
build:
	@echo Platform: $(BOARD)
	@cd kernel && make build
	@echo "Updated: $(KERNEL_ELF)"

PHONY += run
run: build
	@$(QEMU) $(QEMU_ARGS)

PHONY += clean
clean:
	@cargo clean
	@rm -rf $(TARGET_DIR)/*

PHONY += disasm
disasm: $(KERNEL_ASM)
	@cat $(KERNEL_ASM) | $(PAGER)

PHONY += gdbserver
gdbserver:
	@$(QEMU) $(QEMU_ARGS) -s -S

PHONY += gdbclient
gdbclient:
	@$(GDB) -ex 'file $(KERNEL_ELF)' \
			-ex 'set arch riscv:rv64' \
			-ex 'target remote localhost:1234'

.PHONY: $(PHONY)
