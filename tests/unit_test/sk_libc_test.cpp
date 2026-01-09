/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <gtest/gtest.h>

// Rename functions to avoid conflict with standard library
#define atof sk_atof
#define atoi sk_atoi
#define atol sk_atol
#define atoll sk_atoll
#define strtod sk_strtod
#define strtof sk_strtof
#define strtold sk_strtold
#define strtol sk_strtol
#define strtoll sk_strtoll
#define strtoul sk_strtoul
#define strtoull sk_strtoull

#include "../../src/libc/sk_stdlib.c"

TEST(SkLibcTest, Atoi) {
  EXPECT_EQ(sk_atoi("123"), 123);
  EXPECT_EQ(sk_atoi("-123"), -123);
  EXPECT_EQ(sk_atoi("+123"), 123);
  EXPECT_EQ(sk_atoi("0"), 0);
  EXPECT_EQ(sk_atoi("   456"), 456);  // Leading spaces
  EXPECT_EQ(sk_atoi("789abc"), 789);  // Stop at non-digit
}

TEST(SkLibcTest, Atol) {
  EXPECT_EQ(sk_atol("123456789"), 123456789L);
  EXPECT_EQ(sk_atol("-123456789"), -123456789L);
}

TEST(SkLibcTest, Atoll) {
  EXPECT_EQ(sk_atoll("123456789012345"), 123456789012345LL);
  EXPECT_EQ(sk_atoll("-123456789012345"), -123456789012345LL);
}

TEST(SkLibcTest, Atof) {
  EXPECT_DOUBLE_EQ(sk_atof("3.14"), 3.14);
  EXPECT_DOUBLE_EQ(sk_atof("-2.5"), -2.5);
  EXPECT_DOUBLE_EQ(sk_atof("0.0"), 0.0);
  EXPECT_DOUBLE_EQ(sk_atof("123.456"), 123.456);
}

TEST(SkLibcTest, Strtol) {
  char *end;
  EXPECT_EQ(sk_strtol("123", &end, 10), 123L);
  EXPECT_EQ(*end, '\0');

  EXPECT_EQ(sk_strtol("-123", &end, 10), -123L);
  EXPECT_EQ(sk_strtol("   100", &end, 10), 100L);
  EXPECT_EQ(*end, '\0');

  // Base 16
  EXPECT_EQ(sk_strtol("0xABC", &end, 16), 2748L);
  EXPECT_EQ(*end, '\0');
  EXPECT_EQ(sk_strtol("-0xabc", &end, 16), -2748L);

  // Auto detect base
  EXPECT_EQ(sk_strtol("0x10", &end, 0), 16L);
  EXPECT_EQ(sk_strtol("010", &end, 0), 8L);
  EXPECT_EQ(sk_strtol("10", &end, 0), 10L);

  // Stop at invalid
  EXPECT_EQ(sk_strtol("123xyz", &end, 10), 123L);
  EXPECT_EQ(*end, 'x');
}

TEST(SkLibcTest, Strtoul) {
  char *end;
  EXPECT_EQ(sk_strtoul("123", &end, 10), 123UL);
  EXPECT_EQ(sk_strtoul("0xFF", &end, 16), 255UL);
  EXPECT_EQ(sk_strtoul("11", &end, 2), 3UL);
}

TEST(SkLibcTest, Strtoll) {
  char *end;
  // min for 64bit signed
  // -9223372036854775808
  EXPECT_EQ(sk_strtoll("-9223372036854775808", &end, 10),
            -9223372036854775807LL - 1);
}

TEST(SkLibcTest, Strtoull) {
  char *end;
  // max for 64bit unsigned
  // 18446744073709551615
  EXPECT_EQ(sk_strtoull("18446744073709551615", &end, 10),
            18446744073709551615ULL);
}

TEST(SkLibcTest, Strtod) {
  char *end;
  EXPECT_DOUBLE_EQ(sk_strtod("3.14159", &end), 3.14159);
  EXPECT_EQ(*end, '\0');
  EXPECT_DOUBLE_EQ(sk_strtod("  -123.456abc", &end), -123.456);
  EXPECT_EQ(*end, 'a');
}

TEST(SkLibcTest, Strtof) {
  char *end;
  EXPECT_FLOAT_EQ(sk_strtof("3.14", &end), 3.14f);
}
