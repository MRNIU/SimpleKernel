/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief C++ 输入输出
 */

#include "sk_iostream"

#include <cstdint>

#include "sk_cstdio"

namespace sk_std {

ostream& ostream::operator<<(int8_t val) {
  sk_printf("%d", val);
  return *this;
}

ostream& ostream::operator<<(uint8_t val) {
  sk_printf("%d", val);
  return *this;
}

ostream& ostream::operator<<(const char* val) {
  sk_printf("%s", val);
  return *this;
}

ostream& ostream::operator<<(int16_t val) {
  sk_printf("%d", val);
  return *this;
}

ostream& ostream::operator<<(uint16_t val) {
  sk_printf("%d", val);
  return *this;
}

ostream& ostream::operator<<(int32_t val) {
  sk_printf("%d", val);
  return *this;
}

ostream& ostream::operator<<(uint32_t val) {
  sk_printf("%d", val);
  return *this;
}

ostream& ostream::operator<<(int64_t val) {
  sk_printf("%ld", val);
  return *this;
}

ostream& ostream::operator<<(uint64_t val) {
  sk_printf("%ld", val);
  return *this;
}

ostream& ostream::operator<<(ostream& (*manip)(ostream&)) {
  return manip(*this);
}

};  // namespace sk_std
