/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include "sk_iostream"

#include <cstdint>

#include "sk_cstdio"

namespace sk_std {

auto ostream::operator<<(int8_t val) -> ostream& {
  sk_printf("%d", val);
  return *this;
}

auto ostream::operator<<(uint8_t val) -> ostream& {
  sk_printf("%d", val);
  return *this;
}

auto ostream::operator<<(const char* val) -> ostream& {
  sk_printf("%s", val);
  return *this;
}

auto ostream::operator<<(int16_t val) -> ostream& {
  sk_printf("%d", val);
  return *this;
}

auto ostream::operator<<(uint16_t val) -> ostream& {
  sk_printf("%d", val);
  return *this;
}

auto ostream::operator<<(int32_t val) -> ostream& {
  sk_printf("%d", val);
  return *this;
}

auto ostream::operator<<(uint32_t val) -> ostream& {
  sk_printf("%d", val);
  return *this;
}

auto ostream::operator<<(int64_t val) -> ostream& {
  sk_printf("%ld", val);
  return *this;
}

auto ostream::operator<<(uint64_t val) -> ostream& {
  sk_printf("%ld", val);
  return *this;
}

auto ostream::operator<<(ostream& (*manip)(ostream&)) -> ostream& {
  return manip(*this);
}

}  // namespace sk_std
