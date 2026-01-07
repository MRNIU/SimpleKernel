/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "macro.S"

.section .text
.global trap_entry
.type trap_entry, @function
.extern HandleTrap
trap_entry:
    SaveTrapContext
    
    mv a0, sp
    call HandleTrap
    
    RestoreTrapContext
    sret
