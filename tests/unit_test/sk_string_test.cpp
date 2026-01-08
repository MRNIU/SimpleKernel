/**
 * @copyright Copyright The SimpleKernel Contributors
 * @brief string test
 */

#include <gtest/gtest.h>

// Rename functions to avoid conflict with standard library
#define memcpy sk_memcpy
#define memmove sk_memmove
#define memset sk_memset
#define memcmp sk_memcmp
#define memchr sk_memchr
#define strcpy sk_strcpy
#define strncpy sk_strncpy
#define strcat sk_strcat
#define strcmp sk_strcmp
#define strncmp sk_strncmp
#define strlen sk_strlen
#define strnlen sk_strnlen
#define strchr sk_strchr
#define strrchr sk_strrchr

// Include the source file directly to test the implementation
// We need to use extern "C" because the included file is C
// but the functions inside are already wrapped in extern "C" in the .h file
// However, the .c file implementation is compiling as C++ here because the test
// file is .cpp The .c file content: #include "sk_string.h" extern "C" {
// ...
// }
// This structure is fine for C++ compilation too.

#include "../../src/libc/sk_string.c"

TEST(SkStringTest, Memcpy) {
  char src[] = "hello";
  char dest[10];
  sk_memcpy(dest, src, 6);
  EXPECT_STREQ(dest, "hello");
}

TEST(SkStringTest, Memmove) {
  char str[] = "memory move test";
  // Overlap: dest > src
  sk_memmove(str + 7, str, 6);  // "memory " -> "memory " at pos 7
  // "memory memory test"
  EXPECT_STREQ(str, "memory memory test");

  char str2[] = "memory move test";
  // Overlap: dest < src
  sk_memmove(str2, str2 + 7, 4);  // "move" -> starts at 0
  // "move ry move test"
  EXPECT_EQ(str2[0], 'm');
  EXPECT_EQ(str2[1], 'o');
  EXPECT_EQ(str2[2], 'v');
  EXPECT_EQ(str2[3], 'e');
}

TEST(SkStringTest, Memset) {
  char buffer[10];
  sk_memset(buffer, 'A', 5);
  for (int i = 0; i < 5; ++i) {
    EXPECT_EQ(buffer[i], 'A');
  }
}

TEST(SkStringTest, Memcmp) {
  char s1[] = "abc";
  char s2[] = "abc";
  char s3[] = "abd";
  char s4[] = "aba";

  EXPECT_EQ(sk_memcmp(s1, s2, 3), 0);
  EXPECT_LT(sk_memcmp(s1, s3, 3), 0);  // 'c' < 'd' -> 99 - 100 < 0
  EXPECT_GT(sk_memcmp(s1, s4, 3), 0);  // 'c' > 'a'
}

TEST(SkStringTest, Memchr) {
  char s[] = "hello world";
  void* res = sk_memchr(s, 'w', 11);
  EXPECT_EQ(static_cast<char*>(res), &s[6]);

  res = sk_memchr(s, 'z', 11);
  EXPECT_EQ(res, nullptr);
}

TEST(SkStringTest, Strcpy) {
  char src[] = "test";
  char dest[10];
  sk_strcpy(dest, src);
  EXPECT_STREQ(dest, "test");
}

TEST(SkStringTest, Strncpy) {
  char src[] = "test string";
  char dest[20];

  // Normal case
  sk_strncpy(dest, src, 4);
  // manual null termination check if expected,
  // but standard strncpy does NOT null terminate if limit reached?
  // sk_string.c implementation should be checked.
  // Usually strncpy pads with nulls if n > src_len, and does NOT null terminate
  // if n <= src_len

  // Let's verify behavior with simpler test first
  sk_memset(dest, 0, 20);
  sk_strncpy(dest, "abc", 5);
  EXPECT_STREQ(dest, "abc");

  sk_strncpy(dest, "abcdef", 3);
  EXPECT_EQ(dest[0], 'a');
  EXPECT_EQ(dest[1], 'b');
  EXPECT_EQ(dest[2], 'c');
  // dest[3] should remain 0 from memset
  EXPECT_EQ(dest[3], '\0');
}

TEST(SkStringTest, Strcat) {
  char dest[20] = "hello";
  sk_strcat(dest, " world");
  EXPECT_STREQ(dest, "hello world");
}

TEST(SkStringTest, Strcmp) {
  EXPECT_EQ(sk_strcmp("abc", "abc"), 0);
  EXPECT_LT(sk_strcmp("abc", "abd"), 0);
  EXPECT_GT(sk_strcmp("abc", "aba"), 0);
  EXPECT_LT(sk_strcmp("abc", "abcd"), 0);
}

TEST(SkStringTest, Strncmp) {
  EXPECT_EQ(sk_strncmp("abc", "abd", 2), 0);  // "ab" vs "ab"
  EXPECT_LT(sk_strncmp("abc", "abd", 3), 0);
}

TEST(SkStringTest, Strlen) {
  EXPECT_EQ(sk_strlen("hello"), 5);
  EXPECT_EQ(sk_strlen(""), 0);
}

TEST(SkStringTest, Strnlen) {
  EXPECT_EQ(sk_strnlen("hello", 10), 5);
  EXPECT_EQ(sk_strnlen("hello", 3), 3);
}

TEST(SkStringTest, Strchr) {
  char s[] = "hello";
  EXPECT_STREQ(sk_strchr(s, 'e'), "ello");
  EXPECT_EQ(sk_strchr(s, 'z'), nullptr);
  EXPECT_STREQ(sk_strchr(s, 'l'), "llo");  // first 'l'
}

TEST(SkStringTest, Strrchr) {
  char s[] = "hello";
  EXPECT_STREQ(sk_strrchr(s, 'l'), "lo");  // last 'l'
  EXPECT_EQ(sk_strrchr(s, 'z'), nullptr);
}
