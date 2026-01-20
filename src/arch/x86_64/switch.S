/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "macro.S"

.section .text

.global switch_to
.type switch_to, @function

switch_to:

.global kernel_thread_entry
.type kernel_thread_entry, @function
kernel_thread_entry:
