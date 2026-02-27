/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief ETL configuration for SimpleKernel freestanding environment.
 */

#ifndef SIMPLEKERNEL_SRC_INCLUDE_ETL_PROFILE_H_
#define SIMPLEKERNEL_SRC_INCLUDE_ETL_PROFILE_H_

// Use generic C++23 profile as base
#define ETL_CPP23_SUPPORTED 1

// No exceptions in kernel
#define ETL_NO_CHECKS 0

#define ETL_NO_EXCEPTIONS 1

#endif  // SIMPLEKERNEL_SRC_INCLUDE_ETL_PROFILE_H_
