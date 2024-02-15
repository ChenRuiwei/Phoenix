# Building
PACKAGE_NAME=kernel
TARGET := riscv64gc-unknown-none-elf
MODE := debug
KERNEL_ELF := target/$(TARGET)/$(MODE)/$(PACKAGE_NAME)
DISASM_ARGS_TMP := target/$(TARGET)/$(MODE)/asm

# Building mode argument
ifeq ($(MODE), release)
	MODE_ARG := --release
endif

# Board
BOARD := qemu
BOOTLOADER := default

# Tools
TOOLPREFIX := riscv64-elf-
QEMU := qemu-system-riscv64
GDB := $(TOOLPREFIX)gdb
OBJDUMP := rust-objdump --arch-name=riscv64
OBJCOPY := rust-objcopy --binary-architecture=riscv64

DISASM_ARGS := -x

QEMU_ARGS := -machine virt \
			 -nographic \
			 -bios $(BOOTLOADER) \
			 -kernel $(KERNEL_ELF)

$(KERNEL_ELF): build

PHONY := all
all: build

PHONY += env
env:
	@(cargo install --list | grep "cargo-binutils" > /dev/null 2>&1) || cargo install cargo-binutils

PHONY += build
build: env
	@echo Platform: $(BOARD)
	@cp src/linker-$(BOARD).ld src/linker.ld
	@cargo build $(MODE_ARG)
	@rm src/linker.ld

PHONY += run
run: build
	@$(QEMU) $(QEMU_ARGS)

PHONY += clean
clean:
	@cargo clean

PHONY += disasm
disasm: build
	@$(OBJDUMP) $(DISASM_ARGS) $(KERNEL_ELF) | less

PHONY += disasm-nvim
disasm-nvim: build
	@$(OBJDUMP) $(DISASM_ARGS) $(KERNEL_ELF) > $(DISASM_TMP)
	@nvim $(DISASM_TMP)
	@rm $(DISASM_TMP)

PHONY += gdbserver
gdbserver: build
	@$(QEMU) $(QEMU_ARGS) -s -S

PHONY += gdbclient
gdbclient:
	@$(GDB) -ex 'file $(KERNEL_ELF)' \
		-ex 'set arch riscv:rv64' \
		-ex 'target remote localhost:1234'

.PHONY: $(PHONY)
