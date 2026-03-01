/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "kstd_iostream"

#include <cstdint>

#include "kstd_cstdio"

namespace kstd {

auto ostream::operator<<(int8_t val) -> ostream& {
  sk_emit_sint((long long)val);
  return *this;
}

auto ostream::operator<<(uint8_t val) -> ostream& {
  sk_emit_uint((unsigned long long)val);
  return *this;
}

auto ostream::operator<<(const char* val) -> ostream& {
  sk_emit_str(val);
  return *this;
}

auto ostream::operator<<(int16_t val) -> ostream& {
  sk_emit_sint((long long)val);
  return *this;
}

auto ostream::operator<<(uint16_t val) -> ostream& {
  sk_emit_uint((unsigned long long)val);
  return *this;
}

auto ostream::operator<<(int32_t val) -> ostream& {
  sk_emit_sint((long long)val);
  return *this;
}

auto ostream::operator<<(uint32_t val) -> ostream& {
  sk_emit_uint((unsigned long long)val);
  return *this;
}

auto ostream::operator<<(int64_t val) -> ostream& {
  sk_emit_sint((long long)val);
  return *this;
}

auto ostream::operator<<(uint64_t val) -> ostream& {
  sk_emit_uint((unsigned long long)val);
  return *this;
}

auto ostream::operator<<(ostream& (*manip)(ostream&)) -> ostream& {
  return manip(*this);
}

}  // namespace kstd
