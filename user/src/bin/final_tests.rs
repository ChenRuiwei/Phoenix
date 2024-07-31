#![no_std]
#![no_main]
#![feature(generic_arg_infer)]

extern crate alloc;

use user_lib::{execve, fork, wait, waitpid};

#[macro_use]
extern crate user_lib;

const TESTCASES: [&str; 441] = [
    "time-test",
    "busybox_testcode.sh",
    "lua_testcode.sh",
    "netperf_testcode.sh",
    "libc-bench",
    "libctest_testcode.sh",
    "iozone_testcode.sh",
    "unixbench_testcode.sh",
    "lmbench_testcode.sh",
    "cyclictest_testcode.sh",
    "iperf_testcode.sh",
    "./test-ltp.sh ltp/testcases/bin/abs01",
    "./test-ltp.sh ltp/testcases/bin/accept01",
    "./test-ltp.sh ltp/testcases/bin/alarm02",
    "./test-ltp.sh ltp/testcases/bin/alarm03",
    "./test-ltp.sh ltp/testcases/bin/alarm05",
    "./test-ltp.sh ltp/testcases/bin/alarm06",
    "./test-ltp.sh ltp/testcases/bin/alarm07",
    "./test-ltp.sh ltp/testcases/bin/atof01",
    "./test-ltp.sh ltp/testcases/bin/chdir04",
    "./test-ltp.sh ltp/testcases/bin/chmod01",
    "./test-ltp.sh ltp/testcases/bin/chown01",
    "./test-ltp.sh ltp/testcases/bin/chown02",
    "./test-ltp.sh ltp/testcases/bin/chown05",
    "./test-ltp.sh ltp/testcases/bin/chroot03",
    "./test-ltp.sh ltp/testcases/bin/clock_getres01",
    "./test-ltp.sh ltp/testcases/bin/clock_gettime02",
    "./test-ltp.sh ltp/testcases/bin/clock_nanosleep04",
    "./test-ltp.sh ltp/testcases/bin/close01",
    "./test-ltp.sh ltp/testcases/bin/close02",
    "./test-ltp.sh ltp/testcases/bin/clone01",
    "./test-ltp.sh ltp/testcases/bin/clone04",
    "./test-ltp.sh ltp/testcases/bin/confstr01",
    "./test-ltp.sh ltp/testcases/bin/creat01",
    "./test-ltp.sh ltp/testcases/bin/creat03",
    "./test-ltp.sh ltp/testcases/bin/creat05",
    "./test-ltp.sh ltp/testcases/bin/diotest1",
    "./test-ltp.sh ltp/testcases/bin/diotest3",
    "./test-ltp.sh ltp/testcases/bin/diotest6",
    "./test-ltp.sh ltp/testcases/bin/dirty",
    "./test-ltp.sh ltp/testcases/bin/dup01",
    "./test-ltp.sh ltp/testcases/bin/dup02",
    "./test-ltp.sh ltp/testcases/bin/dup03",
    "./test-ltp.sh ltp/testcases/bin/dup04",
    "./test-ltp.sh ltp/testcases/bin/dup06",
    "./test-ltp.sh ltp/testcases/bin/dup07",
    "./test-ltp.sh ltp/testcases/bin/dup201",
    "./test-ltp.sh ltp/testcases/bin/dup202",
    "./test-ltp.sh ltp/testcases/bin/dup203",
    "./test-ltp.sh ltp/testcases/bin/dup204",
    "./test-ltp.sh ltp/testcases/bin/dup205",
    "./test-ltp.sh ltp/testcases/bin/dup206",
    "./test-ltp.sh ltp/testcases/bin/dup207",
    "./test-ltp.sh ltp/testcases/bin/dup3_01",
    "./test-ltp.sh ltp/testcases/bin/dup3_02",
    "./test-ltp.sh ltp/testcases/bin/epoll_create1_02",
    "./test-ltp.sh ltp/testcases/bin/epoll_ctl01",
    "./test-ltp.sh ltp/testcases/bin/epoll_ctl02",
    "./test-ltp.sh ltp/testcases/bin/epoll_ctl04",
    "./test-ltp.sh ltp/testcases/bin/epoll_ctl05",
    "./test-ltp.sh ltp/testcases/bin/epoll_wait01",
    "./test-ltp.sh ltp/testcases/bin/epoll_wait03",
    "./test-ltp.sh ltp/testcases/bin/epoll_wait04",
    "./test-ltp.sh ltp/testcases/bin/epoll_wait07",
    "./test-ltp.sh ltp/testcases/bin/exit02",
    "./test-ltp.sh ltp/testcases/bin/faccessat01",
    "./test-ltp.sh ltp/testcases/bin/faccessat02",
    "./test-ltp.sh ltp/testcases/bin/fchdir01",
    "./test-ltp.sh ltp/testcases/bin/fchdir02",
    "./test-ltp.sh ltp/testcases/bin/fchmod01",
    "./test-ltp.sh ltp/testcases/bin/fchmodat01",
    "./test-ltp.sh ltp/testcases/bin/fchmodat02",
    "./test-ltp.sh ltp/testcases/bin/fchown05",
    "./test-ltp.sh ltp/testcases/bin/fchown01",
    "./test-ltp.sh ltp/testcases/bin/fchown02",
    "./test-ltp.sh ltp/testcases/bin/fcntl02",
    "./test-ltp.sh ltp/testcases/bin/fcntl02_64",
    "./test-ltp.sh ltp/testcases/bin/fcntl03",
    "./test-ltp.sh ltp/testcases/bin/fcntl03_64",
    "./test-ltp.sh ltp/testcases/bin/fcntl04",
    "./test-ltp.sh ltp/testcases/bin/fcntl04_64",
    "./test-ltp.sh ltp/testcases/bin/fcntl05",
    "./test-ltp.sh ltp/testcases/bin/fcntl05_64",
    "./test-ltp.sh ltp/testcases/bin/fcntl08",
    "./test-ltp.sh ltp/testcases/bin/fcntl08_64",
    "./test-ltp.sh ltp/testcases/bin/fcntl13_64",
    "./test-ltp.sh ltp/testcases/bin/fcntl27",
    "./test-ltp.sh ltp/testcases/bin/fcntl29",
    "./test-ltp.sh ltp/testcases/bin/fcntl29_64",
    "./test-ltp.sh ltp/testcases/bin/fdatasync02",
    "./test-ltp.sh ltp/testcases/bin/fgetxattr03",
    "./test-ltp.sh ltp/testcases/bin/flistxattr01",
    "./test-ltp.sh ltp/testcases/bin/flistxattr02",
    "./test-ltp.sh ltp/testcases/bin/flistxattr03",
    "./test-ltp.sh ltp/testcases/bin/flock01",
    "./test-ltp.sh ltp/testcases/bin/flock04",
    "./test-ltp.sh ltp/testcases/bin/flock06",
    "./test-ltp.sh ltp/testcases/bin/fork01",
    "./test-ltp.sh ltp/testcases/bin/fork03",
    "./test-ltp.sh ltp/testcases/bin/fork05",
    "./test-ltp.sh ltp/testcases/bin/fork07",
    "./test-ltp.sh ltp/testcases/bin/fork08",
    "./test-ltp.sh ltp/testcases/bin/fork09",
    "./test-ltp.sh ltp/testcases/bin/fork10",
    "./test-ltp.sh ltp/testcases/bin/fork_procs",
    "./test-ltp.sh ltp/testcases/bin/fpathconf01",
    "./test-ltp.sh ltp/testcases/bin/fptest01",
    "./test-ltp.sh ltp/testcases/bin/fptest02",
    "./test-ltp.sh ltp/testcases/bin/fs_perms",
    "./test-ltp.sh ltp/testcases/bin/fstat02",
    "./test-ltp.sh ltp/testcases/bin/fstat02_64",
    "./test-ltp.sh ltp/testcases/bin/futex_wait01",
    "./test-ltp.sh ltp/testcases/bin/futex_wait03",
    "./test-ltp.sh ltp/testcases/bin/futex_wait04",
    "./test-ltp.sh ltp/testcases/bin/futex_wake01",
    "./test-ltp.sh ltp/testcases/bin/genload",
    "./test-ltp.sh ltp/testcases/bin/genlog10",
    "./test-ltp.sh ltp/testcases/bin/getcontext01",
    "./test-ltp.sh ltp/testcases/bin/getcwd01",
    "./test-ltp.sh ltp/testcases/bin/getcwd02",
    "./test-ltp.sh ltp/testcases/bin/getdomainname01",
    "./test-ltp.sh ltp/testcases/bin/geteuid01",
    "./test-ltp.sh ltp/testcases/bin/gethostbyname_r01",
    "./test-ltp.sh ltp/testcases/bin/gethostname01",
    "./test-ltp.sh ltp/testcases/bin/gethostname02",
    "./test-ltp.sh ltp/testcases/bin/getitimer01",
    "./test-ltp.sh ltp/testcases/bin/getitimer02",
    "./test-ltp.sh ltp/testcases/bin/getpagesize01",
    "./test-ltp.sh ltp/testcases/bin/getpeername01",
    "./test-ltp.sh ltp/testcases/bin/getpgid02",
    "./test-ltp.sh ltp/testcases/bin/getpgrp01",
    "./test-ltp.sh ltp/testcases/bin/getppid01",
    "./test-ltp.sh ltp/testcases/bin/getpriority01",
    "./test-ltp.sh ltp/testcases/bin/getpriority02",
    "./test-ltp.sh ltp/testcases/bin/getrandom01",
    "./test-ltp.sh ltp/testcases/bin/getrandom02",
    "./test-ltp.sh ltp/testcases/bin/getrandom03",
    "./test-ltp.sh ltp/testcases/bin/getrandom04",
    "./test-ltp.sh ltp/testcases/bin/getrandom05",
    "./test-ltp.sh ltp/testcases/bin/getresgid01",
    "./test-ltp.sh ltp/testcases/bin/getresuid01",
    "./test-ltp.sh ltp/testcases/bin/getrlimit01",
    "./test-ltp.sh ltp/testcases/bin/getrlimit02",
    "./test-ltp.sh ltp/testcases/bin/getrlimit03",
    "./test-ltp.sh ltp/testcases/bin/getrusage01",
    "./test-ltp.sh ltp/testcases/bin/getrusage02",
    "./test-ltp.sh ltp/testcases/bin/getsid02",
    "./test-ltp.sh ltp/testcases/bin/getsockname01",
    "./test-ltp.sh ltp/testcases/bin/getsockopt01",
    "./test-ltp.sh ltp/testcases/bin/gettid02",
    "./test-ltp.sh ltp/testcases/bin/gettimeofday01",
    "./test-ltp.sh ltp/testcases/bin/getuid01",
    "./test-ltp.sh ltp/testcases/bin/in6_01",
    "./test-ltp.sh ltp/testcases/bin/in6_02",
    "./test-ltp.sh ltp/testcases/bin/inotify01",
    "./test-ltp.sh ltp/testcases/bin/inotify04",
    "./test-ltp.sh ltp/testcases/bin/ioctl_ns07",
    "./test-ltp.sh ltp/testcases/bin/ioprio_get01",
    "./test-ltp.sh ltp/testcases/bin/ioprio_set01",
    "./test-ltp.sh ltp/testcases/bin/ioprio_set02",
    "./test-ltp.sh ltp/testcases/bin/ioprio_set03",
    "./test-ltp.sh ltp/testcases/bin/kcmp02",
    "./test-ltp.sh ltp/testcases/bin/keyctl03",
    "./test-ltp.sh ltp/testcases/bin/keyctl04",
    "./test-ltp.sh ltp/testcases/bin/keyctl08",
    "./test-ltp.sh ltp/testcases/bin/kill03",
    "./test-ltp.sh ltp/testcases/bin/kill06",
    "./test-ltp.sh ltp/testcases/bin/kill08",
    "./test-ltp.sh ltp/testcases/bin/lgetxattr01",
    "./test-ltp.sh ltp/testcases/bin/lgetxattr02",
    "./test-ltp.sh ltp/testcases/bin/listen01",
    "./test-ltp.sh ltp/testcases/bin/listxattr01",
    "./test-ltp.sh ltp/testcases/bin/listxattr02",
    "./test-ltp.sh ltp/testcases/bin/listxattr03",
    "./test-ltp.sh ltp/testcases/bin/llistxattr01",
    "./test-ltp.sh ltp/testcases/bin/llistxattr02",
    "./test-ltp.sh ltp/testcases/bin/llistxattr03",
    "./test-ltp.sh ltp/testcases/bin/llseek02",
    "./test-ltp.sh ltp/testcases/bin/llseek03",
    "./test-ltp.sh ltp/testcases/bin/locktests",
    "./test-ltp.sh ltp/testcases/bin/lseek01",
    "./test-ltp.sh ltp/testcases/bin/lseek07",
    "./test-ltp.sh ltp/testcases/bin/lstat01",
    "./test-ltp.sh ltp/testcases/bin/lstat01_64",
    "./test-ltp.sh ltp/testcases/bin/ltpServer",
    "./test-ltp.sh ltp/testcases/bin/madvise03",
    "./test-ltp.sh ltp/testcases/bin/madvise05",
    "./test-ltp.sh ltp/testcases/bin/madvise10",
    "./test-ltp.sh ltp/testcases/bin/mallinfo01",
    "./test-ltp.sh ltp/testcases/bin/mallinfo02",
    "./test-ltp.sh ltp/testcases/bin/mallopt01",
    "./test-ltp.sh ltp/testcases/bin/memcmp01",
    "./test-ltp.sh ltp/testcases/bin/memcpy01",
    "./test-ltp.sh ltp/testcases/bin/memset01",
    "./test-ltp.sh ltp/testcases/bin/mincore02",
    "./test-ltp.sh ltp/testcases/bin/mincore03",
    "./test-ltp.sh ltp/testcases/bin/mincore04",
    "./test-ltp.sh ltp/testcases/bin/mkdir04",
    "./test-ltp.sh ltp/testcases/bin/mkdirat01",
    "./test-ltp.sh ltp/testcases/bin/mknod09",
    "./test-ltp.sh ltp/testcases/bin/mlock01",
    "./test-ltp.sh ltp/testcases/bin/mlock04",
    "./test-ltp.sh ltp/testcases/bin/mlockall01",
    "./test-ltp.sh ltp/testcases/bin/mmap01",
    "./test-ltp.sh ltp/testcases/bin/mmap02",
    "./test-ltp.sh ltp/testcases/bin/mmap06",
    "./test-ltp.sh ltp/testcases/bin/mmap08",
    "./test-ltp.sh ltp/testcases/bin/mmap11",
    "./test-ltp.sh ltp/testcases/bin/mmap17",
    "./test-ltp.sh ltp/testcases/bin/mmap18",
    "./test-ltp.sh ltp/testcases/bin/mmap19",
    "./test-ltp.sh ltp/testcases/bin/mmap2",
    "./test-ltp.sh ltp/testcases/bin/mmap20",
    "./test-ltp.sh ltp/testcases/bin/mmapstress04",
    "./test-ltp.sh ltp/testcases/bin/mmstress_dummy",
    "./test-ltp.sh ltp/testcases/bin/modify_ldt01",
    "./test-ltp.sh ltp/testcases/bin/modify_ldt02",
    "./test-ltp.sh ltp/testcases/bin/modify_ldt03",
    "./test-ltp.sh ltp/testcases/bin/mprotect04",
    "./test-ltp.sh ltp/testcases/bin/mq_notify02",
    "./test-ltp.sh ltp/testcases/bin/mq_timedreceive01",
    "./test-ltp.sh ltp/testcases/bin/mq_timedsend01",
    "./test-ltp.sh ltp/testcases/bin/msgctl01",
    "./test-ltp.sh ltp/testcases/bin/msgctl02",
    "./test-ltp.sh ltp/testcases/bin/msgctl03",
    "./test-ltp.sh ltp/testcases/bin/msgctl12",
    "./test-ltp.sh ltp/testcases/bin/msgget01",
    "./test-ltp.sh ltp/testcases/bin/msgrcv07",
    "./test-ltp.sh ltp/testcases/bin/msgrcv08",
    "./test-ltp.sh ltp/testcases/bin/msync01",
    "./test-ltp.sh ltp/testcases/bin/msync02",
    "./test-ltp.sh ltp/testcases/bin/munlock01",
    "./test-ltp.sh ltp/testcases/bin/munlock02",
    "./test-ltp.sh ltp/testcases/bin/name_to_handle_at02",
    "./test-ltp.sh ltp/testcases/bin/newuname01",
    "./test-ltp.sh ltp/testcases/bin/nextafter01",
    "./test-ltp.sh ltp/testcases/bin/nfs05_make_tree",
    "./test-ltp.sh ltp/testcases/bin/nice01",
    "./test-ltp.sh ltp/testcases/bin/nice02",
    "./test-ltp.sh ltp/testcases/bin/open01",
    "./test-ltp.sh ltp/testcases/bin/open03",
    "./test-ltp.sh ltp/testcases/bin/open04",
    "./test-ltp.sh ltp/testcases/bin/open06",
    "./test-ltp.sh ltp/testcases/bin/open09",
    "./test-ltp.sh ltp/testcases/bin/open11",
    "./test-ltp.sh ltp/testcases/bin/openat01",
    "./test-ltp.sh ltp/testcases/bin/pathconf01",
    "./test-ltp.sh ltp/testcases/bin/personality02",
    "./test-ltp.sh ltp/testcases/bin/pipe01",
    "./test-ltp.sh ltp/testcases/bin/pipe02",
    "./test-ltp.sh ltp/testcases/bin/pipe03",
    "./test-ltp.sh ltp/testcases/bin/pipe04",
    "./test-ltp.sh ltp/testcases/bin/pipe05",
    "./test-ltp.sh ltp/testcases/bin/pipe08",
    "./test-ltp.sh ltp/testcases/bin/pipe09",
    "./test-ltp.sh ltp/testcases/bin/pipe14",
    "./test-ltp.sh ltp/testcases/bin/pipe2_01",
    "./test-ltp.sh ltp/testcases/bin/poll01",
    "./test-ltp.sh ltp/testcases/bin/posix_fadvise02",
    "./test-ltp.sh ltp/testcases/bin/posix_fadvise02_64",
    "./test-ltp.sh ltp/testcases/bin/posix_fadvise04",
    "./test-ltp.sh ltp/testcases/bin/posix_fadvise04_64",
    "./test-ltp.sh ltp/testcases/bin/prctl01",
    "./test-ltp.sh ltp/testcases/bin/prctl02",
    "./test-ltp.sh ltp/testcases/bin/prctl05",
    "./test-ltp.sh ltp/testcases/bin/prctl07",
    "./test-ltp.sh ltp/testcases/bin/prctl08",
    "./test-ltp.sh ltp/testcases/bin/pread02",
    "./test-ltp.sh ltp/testcases/bin/pread02_64",
    "./test-ltp.sh ltp/testcases/bin/preadv01",
    "./test-ltp.sh ltp/testcases/bin/preadv01_64",
    "./test-ltp.sh ltp/testcases/bin/preadv02",
    "./test-ltp.sh ltp/testcases/bin/preadv02_64",
    "./test-ltp.sh ltp/testcases/bin/preadv201",
    "./test-ltp.sh ltp/testcases/bin/preadv201_64",
    "./test-ltp.sh ltp/testcases/bin/preadv202",
    "./test-ltp.sh ltp/testcases/bin/preadv202_64",
    "./test-ltp.sh ltp/testcases/bin/print_caps",
    "./test-ltp.sh ltp/testcases/bin/proc01",
    "./test-ltp.sh ltp/testcases/bin/pselect03",
    "./test-ltp.sh ltp/testcases/bin/pselect03_64",
    "./test-ltp.sh ltp/testcases/bin/pwrite04",
    "./test-ltp.sh ltp/testcases/bin/pwrite04_64",
    "./test-ltp.sh ltp/testcases/bin/pwritev01",
    "./test-ltp.sh ltp/testcases/bin/pwritev01_64",
    "./test-ltp.sh ltp/testcases/bin/pwritev02",
    "./test-ltp.sh ltp/testcases/bin/pwritev02_64",
    "./test-ltp.sh ltp/testcases/bin/pwritev201",
    "./test-ltp.sh ltp/testcases/bin/pwritev201_64",
    "./test-ltp.sh ltp/testcases/bin/pwritev202",
    "./test-ltp.sh ltp/testcases/bin/pwritev202_64",
    "./test-ltp.sh ltp/testcases/bin/read01",
    "./test-ltp.sh ltp/testcases/bin/read03",
    "./test-ltp.sh ltp/testcases/bin/read04",
    "./test-ltp.sh ltp/testcases/bin/readdir01",
    "./test-ltp.sh ltp/testcases/bin/readlinkat02",
    "./test-ltp.sh ltp/testcases/bin/readv01",
    "./test-ltp.sh ltp/testcases/bin/readv02",
    "./test-ltp.sh ltp/testcases/bin/realpath01",
    "./test-ltp.sh ltp/testcases/bin/reboot01",
    "./test-ltp.sh ltp/testcases/bin/recvmmsg01",
    "./test-ltp.sh ltp/testcases/bin/recvmsg02",
    "./test-ltp.sh ltp/testcases/bin/remap_file_pages02",
    "./test-ltp.sh ltp/testcases/bin/rename09",
    "./test-ltp.sh ltp/testcases/bin/request_key01",
    "./test-ltp.sh ltp/testcases/bin/request_key05",
    "./test-ltp.sh ltp/testcases/bin/rmdir01",
    "./test-ltp.sh ltp/testcases/bin/rpc_auth_destroy",
    "./test-ltp.sh ltp/testcases/bin/rpc_authnone_create",
    "./test-ltp.sh ltp/testcases/bin/rpc_authunix_create",
    "./test-ltp.sh ltp/testcases/bin/rpc_authunix_create_default",
    "./test-ltp.sh ltp/testcases/bin/rpc_callrpc_performance",
    "./test-ltp.sh ltp/testcases/bin/rpc_callrpc_scalability",
    "./test-ltp.sh ltp/testcases/bin/rpc_callrpc_stress",
    "./test-ltp.sh ltp/testcases/bin/rpc_clnt_broadcast_performance",
    "./test-ltp.sh ltp/testcases/bin/rpc_clnt_broadcast_scalability",
    "./test-ltp.sh ltp/testcases/bin/rpc_clnt_broadcast_stress",
    "./test-ltp.sh ltp/testcases/bin/rpc_clnt_destroy_stress",
    "./test-ltp.sh ltp/testcases/bin/rpc_clntraw_create",
    "./test-ltp.sh ltp/testcases/bin/rpc_clntraw_create_performance",
    "./test-ltp.sh ltp/testcases/bin/rpc_svc_destroy",
    "./test-ltp.sh ltp/testcases/bin/rpc_svc_destroy_stress",
    "./test-ltp.sh ltp/testcases/bin/rpc_svcfd_create",
    "./test-ltp.sh ltp/testcases/bin/rpc_svcfd_create_limits",
    "./test-ltp.sh ltp/testcases/bin/rpc_svcraw_create",
    "./test-ltp.sh ltp/testcases/bin/rpc_svcraw_create_performance",
    "./test-ltp.sh ltp/testcases/bin/rpc_svctcp_create_performance",
    "./test-ltp.sh ltp/testcases/bin/rpc_svcudp_create_performance",
    "./test-ltp.sh ltp/testcases/bin/rpc_xprt_register",
    "./test-ltp.sh ltp/testcases/bin/rpc_xprt_unregister",
    "./test-ltp.sh ltp/testcases/bin/sbrk02",
    "./test-ltp.sh ltp/testcases/bin/sched_get_priority_max01",
    "./test-ltp.sh ltp/testcases/bin/sched_get_priority_max02",
    "./test-ltp.sh ltp/testcases/bin/sched_get_priority_min01",
    "./test-ltp.sh ltp/testcases/bin/sched_get_priority_min02",
    "./test-ltp.sh ltp/testcases/bin/sched_getparam03",
    "./test-ltp.sh ltp/testcases/bin/sched_getscheduler01",
    "./test-ltp.sh ltp/testcases/bin/sched_getscheduler02",
    "./test-ltp.sh ltp/testcases/bin/sched_rr_get_interval01",
    "./test-ltp.sh ltp/testcases/bin/sched_rr_get_interval02",
    "./test-ltp.sh ltp/testcases/bin/sched_rr_get_interval03",
    "./test-ltp.sh ltp/testcases/bin/sched_setparam01",
    "./test-ltp.sh ltp/testcases/bin/sched_setparam02",
    "./test-ltp.sh ltp/testcases/bin/sched_setparam03",
    "./test-ltp.sh ltp/testcases/bin/sched_setparam04",
    "./test-ltp.sh ltp/testcases/bin/sched_setscheduler01",
    "./test-ltp.sh ltp/testcases/bin/sched_tc2",
    "./test-ltp.sh ltp/testcases/bin/sched_tc3",
    "./test-ltp.sh ltp/testcases/bin/sched_tc4",
    "./test-ltp.sh ltp/testcases/bin/sched_tc5",
    "./test-ltp.sh ltp/testcases/bin/sched_yield01",
    "./test-ltp.sh ltp/testcases/bin/semctl03",
    "./test-ltp.sh ltp/testcases/bin/semctl05",
    "./test-ltp.sh ltp/testcases/bin/semctl07",
    "./test-ltp.sh ltp/testcases/bin/semget01",
    "./test-ltp.sh ltp/testcases/bin/semop01",
    "./test-ltp.sh ltp/testcases/bin/sendfile02",
    "./test-ltp.sh ltp/testcases/bin/sendfile02_64",
    "./test-ltp.sh ltp/testcases/bin/sendfile03",
    "./test-ltp.sh ltp/testcases/bin/sendfile03_64",
    "./test-ltp.sh ltp/testcases/bin/sendfile04",
    "./test-ltp.sh ltp/testcases/bin/sendfile04_64",
    "./test-ltp.sh ltp/testcases/bin/sendfile05",
    "./test-ltp.sh ltp/testcases/bin/sendfile05_64",
    "./test-ltp.sh ltp/testcases/bin/sendfile06",
    "./test-ltp.sh ltp/testcases/bin/sendfile06_64",
    "./test-ltp.sh ltp/testcases/bin/sendfile08",
    "./test-ltp.sh ltp/testcases/bin/sendfile08_64",
    "./test-ltp.sh ltp/testcases/bin/sendmmsg01",
    "./test-ltp.sh ltp/testcases/bin/sendmmsg02",
    "./test-ltp.sh ltp/testcases/bin/set_robust_list01",
    "./test-ltp.sh ltp/testcases/bin/set_tid_address01",
    "./test-ltp.sh ltp/testcases/bin/setdomainname01",
    "./test-ltp.sh ltp/testcases/bin/setdomainname02",
    "./test-ltp.sh ltp/testcases/bin/setfsuid02",
    "./test-ltp.sh ltp/testcases/bin/setgid01",
    "./test-ltp.sh ltp/testcases/bin/setgroups01",
    "./test-ltp.sh ltp/testcases/bin/setgroups02",
    "./test-ltp.sh ltp/testcases/bin/sethostname01",
    "./test-ltp.sh ltp/testcases/bin/sethostname02",
    "./test-ltp.sh ltp/testcases/bin/setitimer02",
    "./test-ltp.sh ltp/testcases/bin/setpgid01",
    "./test-ltp.sh ltp/testcases/bin/setpgid02",
    "./test-ltp.sh ltp/testcases/bin/setpgrp01",
    "./test-ltp.sh ltp/testcases/bin/setregid01",
    "./test-ltp.sh ltp/testcases/bin/setregid04",
    "./test-ltp.sh ltp/testcases/bin/setresgid02",
    "./test-ltp.sh ltp/testcases/bin/setresuid01",
    "./test-ltp.sh ltp/testcases/bin/setresuid03",
    "./test-ltp.sh ltp/testcases/bin/setreuid01",
    "./test-ltp.sh ltp/testcases/bin/setrlimit03",
    "./test-ltp.sh ltp/testcases/bin/setrlimit04",
    "./test-ltp.sh ltp/testcases/bin/setsid01",
    "./test-ltp.sh ltp/testcases/bin/setsockopt01",
    "./test-ltp.sh ltp/testcases/bin/setsockopt03",
    "./test-ltp.sh ltp/testcases/bin/setsockopt04",
    "./test-ltp.sh ltp/testcases/bin/settimeofday02",
    "./test-ltp.sh ltp/testcases/bin/setuid01",
    "./test-ltp.sh ltp/testcases/bin/setxattr02",
    "./test-ltp.sh ltp/testcases/bin/shmat01",
    "./test-ltp.sh ltp/testcases/bin/shmat03",
    "./test-ltp.sh ltp/testcases/bin/shmctl03",
    "./test-ltp.sh ltp/testcases/bin/shmctl07",
    "./test-ltp.sh ltp/testcases/bin/shmctl08",
    "./test-ltp.sh ltp/testcases/bin/shmt02",
    "./test-ltp.sh ltp/testcases/bin/shmt03",
    "./test-ltp.sh ltp/testcases/bin/shmt04",
    "./test-ltp.sh ltp/testcases/bin/shmt06",
    "./test-ltp.sh ltp/testcases/bin/shmt07",
    "./test-ltp.sh ltp/testcases/bin/shmt08",
    "./test-ltp.sh ltp/testcases/bin/sigaction01",
    "./test-ltp.sh ltp/testcases/bin/sigaction02",
    "./test-ltp.sh ltp/testcases/bin/sigaltstack01",
    "./test-ltp.sh ltp/testcases/bin/sigaltstack02",
    "./test-ltp.sh ltp/testcases/bin/signal02",
    "./test-ltp.sh ltp/testcases/bin/signal03",
    "./test-ltp.sh ltp/testcases/bin/signal04",
    "./test-ltp.sh ltp/testcases/bin/signal05",
    "./test-ltp.sh ltp/testcases/bin/time-schedule",
    "./test-ltp.sh ltp/testcases/bin/times01",
    "./test-ltp.sh ltp/testcases/bin/tkill01",
    "./test-ltp.sh ltp/testcases/bin/tkill02",
    "./test-ltp.sh ltp/testcases/bin/truncate02",
    "./test-ltp.sh ltp/testcases/bin/truncate02_64",
    "./test-ltp.sh ltp/testcases/bin/uname01",
    "./test-ltp.sh ltp/testcases/bin/uname02",
    "./test-ltp.sh ltp/testcases/bin/uname04",
    "./test-ltp.sh ltp/testcases/bin/unlink05",
    "./test-ltp.sh ltp/testcases/bin/unlink07",
    "./test-ltp.sh ltp/testcases/bin/unlinkat01",
    "./test-ltp.sh ltp/testcases/bin/wait01",
    "./test-ltp.sh ltp/testcases/bin/wait02",
    "./test-ltp.sh ltp/testcases/bin/wait401",
    "./test-ltp.sh ltp/testcases/bin/wait402",
    "./test-ltp.sh ltp/testcases/bin/waitpid01",
    "./test-ltp.sh ltp/testcases/bin/waitpid03",
    "./test-ltp.sh ltp/testcases/bin/waitpid04",
    "./test-ltp.sh ltp/testcases/bin/write01",
    "./test-ltp.sh ltp/testcases/bin/write06",
    "./test-ltp.sh ltp/testcases/bin/writetest",
    "./test-ltp.sh ltp/testcases/bin/writev01",
];

fn run_cmd(cmd: &str) {
    if fork() == 0 {
        execve(
            "busybox",
            &["busybox", "sh", "-c", cmd],
            &[
                "PATH=/:/bin",
                "LD_LIBRARY_PATH=/:/lib:/lib/glibc/:/lib/musl",
            ],
        );
    } else {
        let mut result: i32 = 0;
        waitpid((-1isize) as usize, &mut result);
    }
}

#[no_mangle]
fn main() -> i32 {
    run_cmd("busybox touch sort.src");
    run_cmd("busybox cp /lib/dlopen_dso.so dlopen_dso.so");
    if fork() == 0 {
        for test in TESTCASES {
            run_cmd(&test);
        }
    } else {
        loop {
            let mut exit_code: i32 = 0;
            let pid = wait(&mut exit_code);
            if pid < 0 {
                break;
            }
        }
    }
    0
}
