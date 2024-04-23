# Building variables
DOCKER_NAME = phoenix
BOARD := qemu
export TARGET = riscv64gc-unknown-none-elf
export MODE = release
export LOG = error


# Tools
OBJDUMP = rust-objdump --arch-name=riscv64
OBJCOPY = rust-objcopy --binary-architecture=riscv64
QEMU = qemu-system-riscv64
RISCV_GDB ?= riscv64-unknown-elf-gdb
PAGER ?= less


# Target files
TARGET_DIR := ./target/$(TARGET)/$(MODE)
VENDOR_DIR := ./third-party/vendor

KERNEL_ELF := $(TARGET_DIR)/kernel
KERNEL_BIN := $(KERNEL_ELF).bin
KERNEL_ASM := $(KERNEL_ELF).asm

USER_APPS_DIR := ./user/src/bin
USER_APPS := $(wildcard $(USER_APPS_DIR)/*.rs)
USER_ELFS := $(patsubst $(USER_APPS_DIR)/%.rs, $(TARGET_DIR)/%, $(USER_APPS))
USER_BINS := $(patsubst $(USER_APPS_DIR)/%.rs, $(TARGET_DIR)/%.bin, $(USER_APPS))

FS_IMG_DIR := ./fs-img
FS_IMG := $(FS_IMG_DIR)/sdcard.img
TEST := 23
ifeq ($(TEST), rootfs)
	TEST_DIR := ./Titanix-rootfs/rootfs
else
	TEST_DIR := ./testcase/$(TEST)
endif

# Crate features
export STRACE :=
export SUBMIT :=
export TMPFS :=
export SMP :=
export PRELIMINARY :=


# Args
DISASM_ARGS = -d

BOOTLOADER := default
CPUS := 2
QEMU_ARGS :=
ifeq ($(SUBMIT), )
	QEMU_ARGS += -m 512M
else
	QEMU_ARGS += -m 128M
endif
QEMU_ARGS += -machine virt
QEMU_ARGS += -nographic
QEMU_ARGS += -smp $(CPUS)
QEMU_ARGS += -kernel $(KERNEL_BIN)
QEMU_ARGS += -bios $(BOOTLOADER)
# QEMU_ARGS += -drive file=$(FS_IMG),if=none,format=raw,id=x0
# QEMU_ARGS += -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0

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


# File targets
$(KERNEL_ASM): $(KERNEL_ELF)
	@$(OBJDUMP) $(DISASM_ARGS) $(KERNEL_ELF) > $(KERNEL_ASM)
	@echo "Updated: $(KERNEL_ASM)"


# Phony targets
PHONY := all
all: build run

PHONY += build_docker
build_docker:
	docker build --network="host" -t ${DOCKER_NAME} .

PHONY += docker
docker:
	docker $(DOCKER_RUN_ARGS)

PHONY += env
env:
	@(cargo install --list | grep "cargo-binutils" > /dev/null 2>&1) || cargo install cargo-binutils
	@cargo vendor $(VENDOR_DIR)

PHONY += fmt
fmt:
	@cargo fmt

PHONY += build
build: fmt user fs-img kernel

PHONY += kernel
kernel:
	@echo "building kernel..."
	@echo Platform: $(BOARD)
	@cd kernel && make build
	@$(OBJCOPY) $(KERNEL_ELF) --strip-all -O binary $(KERNEL_BIN)
	@echo "building kernel finished"

PHONY += user
user:
	@echo "building user..."
	@cd user && make build
	@$(foreach elf, $(USER_ELFS), $(OBJCOPY) $(elf) --strip-all -O binary $(patsubst $(TARGET_DIR)/%, $(TARGET_DIR)/%.bin, $(elf));)
	@cp ./testcase/22/busybox $(TARGET_DIR)/busybox
	@echo "building user finished"

# PHONY += fs-img
# fs-img:
# 	@echo "building fs-img..."
# 	@rm -rf $(FS_IMG)
# 	@mkdir -p $(FS_IMG_DIR)
# 	@dd if=/dev/zero of=$(FS_IMG) count=1363148 bs=1K
# 	@mkfs.vfat -F 32 $(FS_IMG)
# 	@echo "making fatfs image by using $(TEST_DIR)"
# 	@mkdir -p mnt
# 	@mount -t vfat -o user,umask=000,utf8=1 --source $(FS_IMG) --target mnt
# 	@cp -r $(TEST_DIR)/* mnt
# 	@umount mnt
# 	@rm -rf mnt
# 	@chmod -R 777 $(FS_IMG_DIR)
# 	@echo "building fs-img finished"

PHONY += qemu
qemu:
	@echo "start to run kernel in qemu..."
	$(QEMU) $(QEMU_ARGS)

PHONY += run
run: qemu

PHONY += brun
brun: fmt clean user kernel run

PHONY += clean
clean:
	@cargo clean
	@rm -rf $(FS_IMG)

PHONY += disasm
disasm: $(KERNEL_ASM)
	@$(PAGER) $(KERNEL_ASM)

PHONY += trace
trace:
	addr2line -fipe $(KERNEL_ELF) | rustfilt

PHONY += drun
drun: fmt clean user kernel
	$(QEMU) $(QEMU_ARGS) -s -S

PHONY += debug
debug:
	$(QEMU) $(QEMU_ARGS) -s -S

PHONY += gdb
gdb:
	$(RISCV_GDB) -ex 'file $(KERNEL_ELF)' -ex 'set arch riscv:rv64' -ex 'target remote localhost:1234'

.PHONY: $(PHONY)

