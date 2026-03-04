/**
 * @copyright Copyright The SimpleKernel Contributors
 */

/// @file klog_format.cpp
/// @brief Bridge translation unit for etl::vformat_to.
///
/// This file is compiled WITHOUT `-mgeneral-regs-only` so that ETL's
/// floating-point formatting templates can be instantiated.  The kernel
/// never formats FP values at runtime; these instantiations exist only
/// because etl::vformat_to's variant visitor requires all alternatives
/// (including float/double/long double) to be handled.
///
/// Every other translation unit calls the non-template VFormatToN()
/// declared in kernel_log.hpp, avoiding FP template instantiation in
/// files compiled with `-mgeneral-regs-only`.

#include <etl/format.h>

namespace klog::detail {

using WrapperIt = etl::private_format::limit_iterator<char*>;

auto VFormatToN(char* buf, size_t n, etl::string_view fmt,
                etl::format_args<WrapperIt> args) -> char* {
  return etl::vformat_to(WrapperIt(buf, n), fmt, args).get();
}

}  // namespace klog::detail
