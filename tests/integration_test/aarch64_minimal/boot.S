
.global _start
_start:
    ldr x11, =stack_top
    mov sp, x11
    bl main
    b .
