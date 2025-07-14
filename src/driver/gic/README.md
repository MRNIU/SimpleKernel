# gic

aarch64 使用的中断控制器

https://developer.arm.com/documentation/100095/0003/Generic-Interrupt-Controller-CPU-Interface

https://www.kernel.org/doc/Documentation/devicetree/bindings/interrupt-controller/arm%2Cgic-v3.txt

https://github.com/qemu/qemu/blob/master/hw/arm/virt.c

```
intc@8000000 {
		phandle = <0x8005>;
		reg = <0x00 0x8000000 0x00 0x10000 0x00 0x80a0000 0x00 0xf60000>;
		#redistributor-regions = <0x01>;
		compatible = "arm,gic-v3";
		ranges;
		#size-cells = <0x02>;
		#address-cells = <0x02>;
		interrupt-controller;
		#interrupt-cells = <0x03>;

		its@8080000 {
			phandle = <0x8006>;
			reg = <0x00 0x8080000 0x00 0x20000>;
			#msi-cells = <0x01>;
			msi-controller;
			compatible = "arm,gic-v3-its";
		};
	};
```
