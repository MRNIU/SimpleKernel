# Platform-Level Interrupt Controller (plic)

riscv64 外部中断控制器

https://github.com/riscv/riscv-plic-spec/blob/master/riscv-plic-1.0.0.pdf

```
plic@c000000 {
	phandle = <0x05>;
	riscv,ndev = <0x5f>;
	reg = <0x00 0xc000000 0x00 0x600000>;
	interrupts-extended = <0x04 0x0b 0x04 0x09 0x02 0x0b 0x02 0x09>;
	interrupt-controller;
	compatible = "sifive,plic-1.0.0\0riscv,plic0";
	#address-cells = <0x00>;
	#interrupt-cells = <0x01>;
};
```
