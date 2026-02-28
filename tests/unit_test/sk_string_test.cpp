/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <gtest/gtest.h>

// Rename functions to avoid conflict with standard library
#define memcpy sk_memcpy
#define memmove sk_memmove
#define memset sk_memset
#define memcmp sk_memcmp
#define memchr sk_memchr
#define strcpy strcpy
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
  memcpy(dest, src, 6);
  EXPECT_STREQ(dest, "hello");
}

TEST(SkStringTest, Memmove) {
  char str[] = "memory move test";
  // Overlap: dest > src
  memmove(str + 7, str, 6);  // "memory " -> "memory " at pos 7
  // "memory memoryest"
  EXPECT_STREQ(str, "memory memoryest");

  char str2[] = "memory move test";
  // Overlap: dest < src
  memmove(str2, str2 + 7, 4);  // "move" -> starts at 0
  // "move ry move test"
  EXPECT_EQ(str2[0], 'm');
  EXPECT_EQ(str2[1], 'o');
  EXPECT_EQ(str2[2], 'v');
  EXPECT_EQ(str2[3], 'e');
}

TEST(SkStringTest, Memset) {
  char buffer[10];
  memset(buffer, 'A', 5);
  for (int i = 0; i < 5; ++i) {
    EXPECT_EQ(buffer[i], 'A');
  }
}

TEST(SkStringTest, Memcmp) {
  char s1[] = "abc";
  char s2[] = "abc";
  char s3[] = "abd";
  char s4[] = "aba";

  EXPECT_EQ(memcmp(s1, s2, 3), 0);
  EXPECT_LT(memcmp(s1, s3, 3), 0);  // 'c' < 'd' -> 99 - 100 < 0
  EXPECT_GT(memcmp(s1, s4, 3), 0);  // 'c' > 'a'
}

TEST(SkStringTest, Memchr) {
  char s[] = "hello world";
  const void* res = memchr(s, 'w', 11);
  EXPECT_EQ(static_cast<const char*>(res), &s[6]);

  res = memchr(s, 'z', 11);
  EXPECT_EQ(res, nullptr);
}

TEST(SkStringTest, Strcpy) {
  char src[] = "test";
  char dest[10];
  strcpy(dest, src);
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
  memset(dest, 0, 20);
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
  EXPECT_EQ(strcmp("abc", "abc"), 0);
  EXPECT_LT(strcmp("abc", "abd"), 0);
  EXPECT_GT(strcmp("abc", "aba"), 0);
  EXPECT_LT(strcmp("abc", "abcd"), 0);
}

TEST(SkStringTest, Strncmp) {
  EXPECT_EQ(sk_strncmp("abc", "abd", 2), 0);  // "ab" vs "ab"
  EXPECT_LT(sk_strncmp("abc", "abd", 3), 0);
}

TEST(SkStringTest, Strlen) {
  EXPECT_EQ(strlen("hello"), 5);
  EXPECT_EQ(strlen(""), 0);
}

TEST(SkStringTest, Strnlen) {
  EXPECT_EQ(strnlen("hello", 10), 5);
  EXPECT_EQ(strnlen("hello", 3), 3);
}

TEST(SkStringTest, Strchr) {
  char s[] = "hello";
  EXPECT_STREQ(strchr(s, 'e'), "ello");
  EXPECT_EQ(strchr(s, 'z'), nullptr);
  EXPECT_STREQ(strchr(s, 'l'), "llo");  // first 'l'
}

TEST(SkStringTest, Strrchr) {
  char s[] = "hello";
  EXPECT_STREQ(strrchr(s, 'l'), "lo");  // last 'l'
  EXPECT_EQ(strrchr(s, 'z'), nullptr);
}

// 边界条件测试
TEST(SkStringTest, MemcpyEdgeCases) {
  char src[] = "test";
  char dest[10];

  // 零长度复制
  memcpy(dest, src, 0);

  // 单字节复制
  memcpy(dest, src, 1);
  EXPECT_EQ(dest[0], 't');
}

TEST(SkStringTest, MemsetEdgeCases) {
  char buffer[10];

  // 零长度设置
  memset(buffer, 'A', 0);

  // 使用 0 填充
  memset(buffer, 0, 5);
  for (int i = 0; i < 5; ++i) {
    EXPECT_EQ(buffer[i], 0);
  }

  // 使用负值填充 (转换为 unsigned char)
  memset(buffer, -1, 3);
  for (int i = 0; i < 3; ++i) {
    EXPECT_EQ(static_cast<unsigned char>(buffer[i]), 255);
  }
}

TEST(SkStringTest, StrcmpEdgeCases) {
  // 空字符串比较
  EXPECT_EQ(strcmp("", ""), 0);
  EXPECT_LT(strcmp("", "a"), 0);
  EXPECT_GT(strcmp("a", ""), 0);

  // 一个字符串是另一个的前缀
  EXPECT_LT(strcmp("abc", "abcd"), 0);
  EXPECT_GT(strcmp("abcd", "abc"), 0);
}

TEST(SkStringTest, StrlenEdgeCases) {
  // 空字符串
  EXPECT_EQ(strlen(""), 0);

  // 只有空字符
  const char null_str[] = {'\0', 'a', 'b', '\0'};
  EXPECT_EQ(strlen(null_str), 0);
}

TEST(SkStringTest, StrnlenEdgeCases) {
  // n 为 0
  EXPECT_EQ(strnlen("hello", 0), 0);

  // n 大于字符串长度
  EXPECT_EQ(strnlen("hi", 100), 2);

  // n 等于字符串长度
  EXPECT_EQ(strnlen("hello", 5), 5);
}

TEST(SkStringTest, StrchrEdgeCases) {
  char s[] = "hello";

  // 查找空字符
  EXPECT_EQ(strchr(s, '\0'), &s[5]);

  // 第一个字符
  EXPECT_EQ(strchr(s, 'h'), s);
}

TEST(SkStringTest, StrrchrEdgeCases) {
  char s[] = "hello";

  // 查找空字符
  EXPECT_EQ(strrchr(s, '\0'), &s[5]);

  // 第一个也是最后一个
  EXPECT_EQ(strrchr(s, 'h'), s);
}

TEST(SkStringTest, MemmoveOverlapForward) {
  // 测试前向重叠: dest > src
  char str[] = "1234567890";
  memmove(str + 3, str, 5);  // "12312345890"
  EXPECT_EQ(str[3], '1');
  EXPECT_EQ(str[4], '2');
  EXPECT_EQ(str[5], '3');
  EXPECT_EQ(str[6], '4');
  EXPECT_EQ(str[7], '5');
}

TEST(SkStringTest, MemmoveOverlapBackward) {
  // 测试后向重叠: dest < src
  char str[] = "1234567890";
  memmove(str, str + 3, 5);  // "4567567890"
  EXPECT_EQ(str[0], '4');
  EXPECT_EQ(str[1], '5');
  EXPECT_EQ(str[2], '6');
  EXPECT_EQ(str[3], '7');
  EXPECT_EQ(str[4], '8');
}

TEST(SkStringTest, MemmoveNoOverlap) {
  // 无重叠
  char src[] = "source";
  char dest[10];
  memmove(dest, src, 7);
  EXPECT_STREQ(dest, "source");
}

TEST(SkStringTest, MemchrNotFound) {
  char s[] = "hello world";
  EXPECT_EQ(memchr(s, 'x', 11), nullptr);
  EXPECT_EQ(memchr(s, 'z', 11), nullptr);
}

TEST(SkStringTest, MemcmpEqual) {
  char s1[] = "test";
  char s2[] = "test";
  EXPECT_EQ(memcmp(s1, s2, 4), 0);
}

TEST(SkStringTest, MemcmpDifferentLengths) {
  char s1[] = "abc";
  char s2[] = "abcd";
  // 只比较前3个字节
  EXPECT_EQ(memcmp(s1, s2, 3), 0);
}

TEST(SkStringTest, StrcatMultiple) {
  char dest[30] = "hello";
  sk_strcat(dest, " ");
  sk_strcat(dest, "world");
  sk_strcat(dest, "!");
  EXPECT_STREQ(dest, "hello world!");
}

TEST(SkStringTest, StrncpyPadding) {
  char dest[10];
  memset(dest, 'X', 10);  // 用 'X' 填充

  // 复制短字符串，应该填充空字符
  sk_strncpy(dest, "ab", 5);
  EXPECT_EQ(dest[0], 'a');
  EXPECT_EQ(dest[1], 'b');
  EXPECT_EQ(dest[2], '\0');
  EXPECT_EQ(dest[3], '\0');
  EXPECT_EQ(dest[4], '\0');
}
