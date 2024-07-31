![哈工大深圳](./docs/assets/hitsz-logo.jpg)

# Phoenix

## 项目描述

Phoenix 是使用 Rust 编写、基于 RISCV-64 硬件平台、支持多核、采用异步无栈协程架构的模块化宏内核操作系统。

## 完成情况

### 决赛第一阶段

VisionFive 2 赛道，通过了除部分ltp测试外的所有测试点，排名：

![初赛排行榜](./docs/assets/leaderboard-pre.png)

### 初赛

VisionFive 2 赛道，初赛功能测试满分：

![初赛排行榜](./docs/assets/leaderboard-pre.png)

### Phoenix 内核介绍

- 无栈协程：基于全局队列实现的调度器，完善的辅助 Future 支持，支持内核态抢占式调度。
- 进程管理：统一的进程线程抽象，可以细粒度划分进程共享的资源，支持多核运行。
- 内存管理：实现基本的内存管理功能。使用懒分配和 Copy-on-Write 优化策略。
- 文件系统：基于 Linux 设计的虚拟文件系统。实现页缓存加速文件读写，实现 Dentry 缓存加速路径查找，统一了页缓存与块缓存。使用开源 `rust-fatfs`库提供对 FAT32 文件系统的支持，使用`lwext4-rust`库提供对Ext4文件系统的支持。
- 信号机制：支持用户自定义信号处理例程，有完善的信号系统，与内核其他异步设施无缝衔接。
- 设备驱动：实现设备树解析，实现PLIC，支持异步外设中断，实现异步串口驱动。
- 网络模块：支持Udp和Tcp套接字，Ipv4与Ipv6协议，实现异步轮询唤醒机制。

### 文档

[Phoenix-初赛文档](./Phoenix-初赛文档.pdf)

### 项目结构

```
.
├── arch                    # 平台相关的包装函数与启动函数
├── config                  # 配置常量
├── crates                  # 自己编写的功能单一的库
│   ├── backtrace           # 堆栈回溯
│   ├── macro-utils         # 宏工具
│   ├── range-map           # 范围映射
│   ├── recycle-allocator   # ID回收分配器
│   ├── ring-buffer         # 环形队列缓冲区
│   └── sbi-print           # SBI打印工具
├── docs                    # 文档
├── driver                  # 驱动模块
├── kernel                  # 内核
│   ├── src
│   │   ├── ipc             # 进程间通信机制
│   │   ├── mm              # 内存管理
│   │   ├── net             # 网络
│   │   ├── processor       # 多核心管理
│   │   ├── syscall         # 系统调用
│   │   ├── task            # 进程管理
│   │   ├── trap            # 异常处理
│   │   ├── utils           # 工具
│   │   ├── boot.rs         # 内核启动通用函数
│   │   ├── impls.rs        # 模块接口实现
│   │   ├── main.rs         # 主函数
│   │   ├── panic.rs
│   │   └── trampoline.asm  # 信号跳板
│   ├── build.rs
│   ├── Cargo.toml
│   ├── linker.ld           # 链接脚本
│   └── Makefile
├── modules                 # 内核各个模块
│   ├── device-core         # 设备API
│   ├── executor            # 异步调度器
│   ├── ext4                # Ext4文件系统支持
│   ├── fat32               # FAT32文件系统支持
│   ├── logging             # 日志系统
│   ├── memory              # 基础内存模块
│   ├── net                 # 基础信号模块
│   ├── page                # 页缓存与块缓存
│   ├── signal              # 基础信号模块
│   ├── sync                # 同步原语
│   ├── systype             # 系统调用类型
│   ├── time                # 时间模块
│   ├── timer               # 定时器模块
│   ├── vfs                 # 虚拟文件系统模块
│   └── vfs-core            # 虚拟文件系统接口
├── testcase                # 测试用例
├── third-party             # 第三方库
│   └── vendor              # Rust库缓存
├── user                    # 用户程序
├── Cargo.lock
├── Cargo.toml
├── Dockerfile
├── LICENSE
├── Makefile
├── README.md
├── rustfmt.toml
└── rust-toolchain.toml
```

## 运行

1. 在项目根目录下，进入 root 用户，构建`docker`容器

```sh
make build_docker
```

2. 运行容器，进入容器终端

```sh
make docker
```

3. 下载依赖库并缓存在 `third-party/vendor` 文件夹下

```sh
make env
```

4. 编译内核，烧录文件镜像，并在`Qemu`中运行内核

```sh
make all
```

## 项目人员

哈尔滨工业大学（深圳）:

- 陈睿玮 (<1982833213@qq.com>)
- 石全 (<749990226@qq.com>)
- 王华杰 (<1070001239@qq.com>)
- 指导老师：夏文，仇洁婷

## 参考

- [Titanix](https://gitlab.eduxiji.net/202318123101314/oskernel2023-Titanix) 启动流程、内存模块部分设计
- [MankorOS](https://gitlab.eduxiji.net/MankorOS/OSKernel2023-MankorOS) RangeMap、UserPtr、设备树解析、部分驱动
- [FTL OS](https://gitlab.eduxiji.net/DarkAngelEX/oskernel2022-ftlos)
- [ArceOS](https://github.com/arceos-org/arceos) 网络模块部分设计
- [Alien](https://gitlab.eduxiji.net/202310007101563/Alien) 虚拟文件系统部分设计
