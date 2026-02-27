/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief Pool infrastructure for kstd containers.
 *
 * Provides pool-backed allocation using ETL's ipool/pool<T,N>.
 * All ETL details are hidden from consumers.
 *
 * @note Only typed pools (etl::pool<T,N>) are used.
 *       etl::generic_pool is NOT used anywhere.
 */

#ifndef SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_KSTD_POOL_HPP_
#define SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_KSTD_POOL_HPP_

#include <cstddef>

#include "etl/ipool.h"
#include "etl/pool.h"

namespace kstd {

/// @brief Type-erased pool reference. Consumers see this, not etl::ipool.
using PoolRef = etl::ipool;

/// @brief Fixed-capacity typed object pool with static storage.
/// @tparam T Element type.
/// @tparam N Maximum number of elements.
template <typename T, size_t N>
using Pool = etl::pool<T, N>;

}  // namespace kstd

#endif  // SIMPLEKERNEL_SRC_LIBCXX_INCLUDE_KSTD_POOL_HPP_
