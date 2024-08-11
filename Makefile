# Building variables
DOCKER_NAME = phoenix
BOARD := qemu

NET ?=n # 是否启用VirtioNet设备，如果不开启则使用本地Loopback设备

export TARGET = riscv64gc-unknown-none-elf
export MODE = release
export LOG = error

export Phoenix_IP=$(IP)
export Phoenix_GW=$(GW)

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

FS_IMG_DIR := .
FS_IMG := $(FS_IMG_DIR)/sdcard.img
TEST := 24/final
# FS := fat32
FS := ext4
TEST_DIR := ./testcase/$(TEST)
# TEST_DIR := ./testcase/24/preliminary/

# Crate features
export STRACE := 
export SMP :=
export PREEMPT :=
export DEBUG :=

# Args
DISASM_ARGS = -d

BOOTLOADER := default
CPUS := 2
QEMU_ARGS :=
QEMU_ARGS += -m 128M
QEMU_ARGS += -machine virt
QEMU_ARGS += -nographic
QEMU_ARGS += -smp $(CPUS)
QEMU_ARGS += -kernel $(KERNEL_BIN)
QEMU_ARGS += -bios $(BOOTLOADER)
QEMU_ARGS += -drive file=$(FS_IMG),if=none,format=raw,id=x0
QEMU_ARGS += -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0

# Net
IP ?= 10.0.2.15
GW ?= 10.0.2.2

ifeq ($(NET),y)
$(info "enabled qemu net device")
# 指定该网络设备使用 net0 这个网络后端，使用用户模式网络。
# 设置端口转发，将主机的 TCP 端口 5555 和 UDP 端口 5555 分别转发到虚拟机的 TCP端口 5555 和 UDP 端口 5555。
QEMU_ARGS += -device virtio-net-device,netdev=net0 \
             -netdev user,id=net0,hostfwd=tcp::5555-:5555,hostfwd=udp::5555-:5555
QEMU_ARGS += -d guest_errors\
			 -d unimp

endif

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
all: build run MODE=release

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

PHONY += fs-img
fs-img:
	@echo "building fs-img..."
	@rm -f $(FS_IMG)
	@mkdir -p $(FS_IMG_DIR)
	@mkdir -p mnt
ifeq ($(FS), fat32)
	@dd if=/dev/zero of=$(FS_IMG) count=1363148 bs=1K
	@mkfs.vfat -F 32 -s 8 $(FS_IMG)
	@echo "making fatfs image by using $(TEST_DIR)"
	@mount -t vfat -o user,umask=000,utf8=1 --source $(FS_IMG) --target mnt
else
	@dd if=/dev/zero of=$(FS_IMG) count=2048 bs=1M
	# @mkfs.ext4 $(FS_IMG)
	@mkfs.ext4  -F -O ^metadata_csum_seed $(FS_IMG)
	@echo "making ext4 image by using $(TEST_DIR)"
	@mount $(FS_IMG) mnt
endif
	@cp -r $(TEST_DIR)/* mnt
	@cp -r $(USER_ELFS) mnt
	@umount mnt
	@rm -rf mnt
	@chmod 777 $(FS_IMG)
	@echo "building fs-img finished"

PHONY += qemu
qemu:
	@echo "start to run kernel in qemu..."
	$(QEMU) $(QEMU_ARGS)

PHONY += dumpdtb
dumpdtb:
	$(QEMU) $(QEMU_ARGS) -machine dumpdtb=riscv64-virt.dtb
	dtc -I dtb -O dts -o riscv64-virt.dts riscv64-virt.dtb

PHONY += run
run: qemu

PHONY += brun
brun: fmt clean-cargo user kernel run

PHONY += clean
clean:
	@cargo clean
	@rm -rf $(FS_IMG)

PHONY += clean-cargo
clean-cargo:
	@cargo clean

PHONY += disasm
disasm: $(KERNEL_ASM)
	@$(PAGER) $(KERNEL_ASM)

PHONY += trace
trace:
	addr2line -fipe $(KERNEL_ELF) | rustfilt

PHONY += drun
drun: fmt clean-cargo user kernel
	$(QEMU) $(QEMU_ARGS) -s -S

PHONY += debug
debug:
	$(QEMU) $(QEMU_ARGS) -s -S

PHONY += gdb
gdb:
	$(RISCV_GDB) -ex 'file $(KERNEL_ELF)' -ex 'set arch riscv:rv64' -ex 'target remote localhost:1234'


.PHONY: $(PHONY)

