# system_test

c++ 构造函数测试

步骤：

1. 在 qemu 中运行
2. 构建全局变量、静态局部变量
3. 抛出 throw

```shell
# aarch64
qemu-system-aarch64 -nographic -serial stdio -monitor telnet::2333,server,nowait -m 1024M -machine virt -cpu cortex-a72 -kernel bin/cxx-init-test
# riscv64
qemu-system-riscv64 -nographic -serial stdio -monitor telnet::2333,server,nowait -machine virt -bios 3rd/opensbi/platform/generic/firmware/fw_jump.elf -kernel bin/cxx-init-test
# x86_64
make u-boot
qemu-system-x86_64 -nographic -serial stdio -monitor telnet::2333,server,nowait -m 128M -net none -bios 3rd/u-boot/u-boot.rom -drive file=fat:rw:bin,format=raw,media=disk
```
