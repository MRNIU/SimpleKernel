# pl011

aarch64 使用的 uart

https://developer.arm.com/documentation/ddi0183/g/

```
pl011@9000000 {
		clock-names = "uartclk\0apb_pclk";
		clocks = <0x8000 0x8000>;
		interrupts = <0x00 0x01 0x04>;
		reg = <0x00 0x9000000 0x00 0x1000>;
		compatible = "arm,pl011\0arm,primecell";
	};
```
