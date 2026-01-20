/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "macro.S"

.global vector_table
.balign 0x800
vector_table:
// Current EL with SP0
.balign 0x80
    b sync_current_el_sp0
.balign 0x80
    b irq_current_el_sp0
.balign 0x80
    b fiq_current_el_sp0
.balign 0x80
    b error_current_el_sp0

// Current EL with SPx
.balign 0x80
    b sync_current_el_spx
.balign 0x80
    b irq_current_el_spx
.balign 0x80
    b fiq_current_el_spx
.balign 0x80
    b error_current_el_spx

// Lower EL using AArch64
.balign 0x80
    b sync_lower_el_aarch64
.balign 0x80
    b irq_lower_el_aarch64
.balign 0x80
    b fiq_lower_el_aarch64
.balign 0x80
    b error_lower_el_aarch64

// Lower EL using AArch32
.balign 0x80
    b sync_lower_el_aarch32
.balign 0x80
    b irq_lower_el_aarch32
.balign 0x80
    b fiq_lower_el_aarch32
.balign 0x80
    b error_lower_el_aarch32

// Exception handlers
sync_current_el_sp0:
    SaveTrapContext
    mov x0, sp
    bl sync_current_el_sp0_handler
    RestoreTrapContext
    eret

irq_current_el_sp0:
    SaveTrapContext
    mov x0, sp
    bl irq_current_el_sp0_handler
    RestoreTrapContext
    eret

fiq_current_el_sp0:
    SaveTrapContext
    mov x0, sp
    bl fiq_current_el_sp0_handler
    RestoreTrapContext
    eret

error_current_el_sp0:
    SaveTrapContext
    mov x0, sp
    bl error_current_el_sp0_handler
    RestoreTrapContext
    eret

sync_current_el_spx:
    SaveTrapContext
    mov x0, sp
    bl sync_current_el_spx_handler
    RestoreTrapContext
    eret

irq_current_el_spx:
    SaveTrapContext
    mov x0, sp
    bl irq_current_el_spx_handler
    RestoreTrapContext
    eret

fiq_current_el_spx:
    SaveTrapContext
    mov x0, sp
    bl fiq_current_el_spx_handler
    RestoreTrapContext
    eret

error_current_el_spx:
    SaveTrapContext
    mov x0, sp
    bl error_current_el_spx_handler
    RestoreTrapContext
    eret

sync_lower_el_aarch64:
    SaveTrapContext
    mov x0, sp
    bl sync_lower_el_aarch64_handler
    RestoreTrapContext
    eret

irq_lower_el_aarch64:
    SaveTrapContext
    mov x0, sp
    bl irq_lower_el_aarch64_handler
    RestoreTrapContext
    eret

fiq_lower_el_aarch64:
    SaveTrapContext
    mov x0, sp
    bl fiq_lower_el_aarch64_handler
    RestoreTrapContext
    eret

error_lower_el_aarch64:
    SaveTrapContext
    mov x0, sp
    bl error_lower_el_aarch64_handler
    RestoreTrapContext
    eret

sync_lower_el_aarch32:
    SaveTrapContext
    mov x0, sp
    bl sync_lower_el_aarch32_handler
    RestoreTrapContext
    eret

irq_lower_el_aarch32:
    SaveTrapContext
    mov x0, sp
    bl irq_lower_el_aarch32_handler
    RestoreTrapContext
    eret

fiq_lower_el_aarch32:
    SaveTrapContext
    mov x0, sp
    bl fiq_lower_el_aarch32_handler
    RestoreTrapContext
    eret

error_lower_el_aarch32:
    SaveTrapContext
    mov x0, sp
    bl error_lower_el_aarch32_handler
    RestoreTrapContext
    eret
