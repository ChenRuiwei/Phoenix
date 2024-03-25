# Phoenix OS

Phoenix OS is a Unix-like OS for 2024 OS-Competition

## 项目描述

Rust编写的宏内核操作系统，基于RISC-V64硬件平台，支持多核

## 运行

如果不在根用户下，先进入根用户

```sh
su
```

建议使用`docker`来编译和运行内核。第一次运行前, 构建`docker`容器，输入

```sh
make build_docker
```

`docker`容器只需要构建一次，之后所有的编译和运行都在容器里进行。

进入容器，输入

```sh
make docker
```

第一次运行内核，需要下载对应Rust工具链并生成vendor文件夹，输入命令

```sh
make env
```

构建并在`Qemu`中运行内核, 输入

```sh
make all
```

单独构建内核, 输入

```sh
make build
```

在`Qemu`中运行内核，输入

```sh
make run
```
