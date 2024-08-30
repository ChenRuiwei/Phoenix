# syntax=docker/dockerfile:1
FROM ubuntu:22.04

ARG QEMU_VERSION=7.0.0
ARG HOME=/root

# 0. Install general tools
ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update && \
    apt-get install -y \
    curl \
    git \
    python3 \
    wget \
    xz-utils

# 1. Set up QEMU RISC-V
# - https://learningos.github.io/rust-based-os-comp2022/0setup-devel-env.html#qemu
# - https://www.qemu.org/download/
# - https://wiki.qemu.org/Documentation/Platforms/RISCV
# - https://risc-v-getting-started-guide.readthedocs.io/en/latest/linux-qemu.html

# 1.1. Download source
WORKDIR ${HOME}
RUN wget --progress=dot:giga https://download.qemu.org/qemu-${QEMU_VERSION}.tar.xz && \
    tar xvJf qemu-${QEMU_VERSION}.tar.xz

# 1.2. Install dependencies
# - https://risc-v-getting-started-guide.readthedocs.io/en/latest/linux-qemu.html#prerequisites
RUN apt-get update && \
    apt-get install -y \
    autoconf automake autotools-dev curl libmpc-dev libmpfr-dev libgmp-dev \
    gawk build-essential bison flex texinfo gperf libtool patchutils bc \
    zlib1g-dev libexpat-dev git \
    ninja-build pkg-config libglib2.0-dev libpixman-1-dev libsdl2-dev \
    dosfstools cmake

# 1.3. Build and install from source
WORKDIR ${HOME}/qemu-${QEMU_VERSION}
RUN ./configure --target-list=riscv64-softmmu,riscv64-linux-user && \
    make -j$(nproc) && \
    make install

# 1.4. Clean up
WORKDIR ${HOME}
RUN rm -rf qemu-${QEMU_VERSION} qemu-${QEMU_VERSION}.tar.xz

# 1.5. Sanity checking
RUN qemu-system-riscv64 --version && \
    qemu-riscv64 --version

# 1.6. Add musl cc
RUN wget --progress=dot:giga https://musl.cc/riscv64-linux-musl-cross.tgz && \
    tar xvf riscv64-linux-musl-cross.tgz
RUN rm -rf riscv64-linux-musl-cross.tgz
ENV PATH=${HOME}/riscv64-linux-musl-cross/bin:$PATH

# 2. Set up Rust
# - https://learningos.github.io/rust-based-os-comp2022/0setup-devel-env.html#qemu
# - https://www.rust-lang.org/tools/install
# - https://github.com/rust-lang/docker-rust/blob/master/Dockerfile-debian.template

# 2.1. Install
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUST_VERSION=nightly-2024-02-03 \
    PROFILE=minimal
RUN set -eux; \
    wget --progress=dot:giga https://sh.rustup.rs -O rustup-init; \
    chmod +x rustup-init; \
    ./rustup-init -y --no-modify-path --profile $PROFILE --default-toolchain $RUST_VERSION; \
    rm rustup-init; \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME;

# 2.2. Sanity checking
RUN rustup --version && \
    cargo --version && \
    rustc --version

# 3. Build env for labs
RUN rustup target add riscv64gc-unknown-none-elf && \
    rustup component add rust-src && \
    rustup component add rustfmt && \
    rustup component add clippy && \
    rustup component add llvm-tools && \
    cargo install cargo-binutils

# Ready to go
WORKDIR ${HOME}
