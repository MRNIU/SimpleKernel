/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <gtest/gtest.h>

// Rename functions to avoid conflict with standard library
#define isalnum sk_isalnum
#define isalpha sk_isalpha
#define isblank sk_isblank
#define iscntrl sk_iscntrl
#define isdigit sk_isdigit
#define isgraph sk_isgraph
#define islower sk_islower
#define isprint sk_isprint
#define ispunct sk_ispunct
#define isspace sk_isspace
#define isupper sk_isupper
#define isxdigit sk_isxdigit
#define tolower sk_tolower
#define toupper sk_toupper

// Include the source file directly to test the implementation
#include "../../src/libc/sk_ctype.c"

TEST(SkCtypeTest, IsAlnum) {
  EXPECT_TRUE(sk_isalnum('a'));
  EXPECT_TRUE(sk_isalnum('Z'));
  EXPECT_TRUE(sk_isalnum('0'));
  EXPECT_TRUE(sk_isalnum('9'));
  EXPECT_FALSE(sk_isalnum('!'));
  EXPECT_FALSE(sk_isalnum(' '));
}

TEST(SkCtypeTest, IsAlpha) {
  EXPECT_TRUE(sk_isalpha('a'));
  EXPECT_TRUE(sk_isalpha('Z'));
  EXPECT_FALSE(sk_isalpha('0'));
  EXPECT_FALSE(sk_isalpha('!'));
}

TEST(SkCtypeTest, IsBlank) {
  EXPECT_TRUE(sk_isblank(' '));
  EXPECT_TRUE(sk_isblank('\t'));
  EXPECT_FALSE(sk_isblank('\n'));
  EXPECT_FALSE(sk_isblank('a'));
}

TEST(SkCtypeTest, IsCntrl) {
  EXPECT_TRUE(sk_iscntrl(0));
  EXPECT_TRUE(sk_iscntrl(31));
  EXPECT_TRUE(sk_iscntrl(127));
  EXPECT_FALSE(sk_iscntrl(' '));
  EXPECT_FALSE(sk_iscntrl('a'));
}

TEST(SkCtypeTest, IsDigit) {
  EXPECT_TRUE(sk_isdigit('0'));
  EXPECT_TRUE(sk_isdigit('9'));
  EXPECT_FALSE(sk_isdigit('a'));
  EXPECT_FALSE(sk_isdigit(' '));
}

TEST(SkCtypeTest, IsGraph) {
  EXPECT_TRUE(sk_isgraph('!'));
  EXPECT_TRUE(sk_isgraph('a'));
  EXPECT_TRUE(sk_isgraph('~'));
  EXPECT_FALSE(sk_isgraph(' '));
  EXPECT_FALSE(sk_isgraph('\n'));
}

TEST(SkCtypeTest, IsLower) {
  EXPECT_TRUE(sk_islower('a'));
  EXPECT_TRUE(sk_islower('z'));
  EXPECT_FALSE(sk_islower('A'));
  EXPECT_FALSE(sk_islower('0'));
}

TEST(SkCtypeTest, IsPrint) {
  EXPECT_TRUE(sk_isprint(' '));
  EXPECT_TRUE(sk_isprint('a'));
  EXPECT_TRUE(sk_isprint('~'));
  EXPECT_FALSE(sk_isprint('\t'));
  EXPECT_FALSE(sk_isprint(31));
}

TEST(SkCtypeTest, IsPunct) {
  EXPECT_TRUE(sk_ispunct('!'));
  EXPECT_TRUE(sk_ispunct('.'));
  EXPECT_FALSE(sk_ispunct('a'));
  EXPECT_FALSE(sk_ispunct('0'));
  EXPECT_FALSE(sk_ispunct(' '));
}

TEST(SkCtypeTest, IsSpace) {
  EXPECT_TRUE(sk_isspace(' '));
  EXPECT_TRUE(sk_isspace('\f'));
  EXPECT_TRUE(sk_isspace('\n'));
  EXPECT_TRUE(sk_isspace('\r'));
  EXPECT_TRUE(sk_isspace('\t'));
  EXPECT_TRUE(sk_isspace('\v'));
  EXPECT_FALSE(sk_isspace('a'));
}

TEST(SkCtypeTest, IsUpper) {
  EXPECT_TRUE(sk_isupper('A'));
  EXPECT_TRUE(sk_isupper('Z'));
  EXPECT_FALSE(sk_isupper('a'));
  EXPECT_FALSE(sk_isupper('0'));
}

TEST(SkCtypeTest, IsXdigit) {
  EXPECT_TRUE(sk_isxdigit('0'));
  EXPECT_TRUE(sk_isxdigit('9'));
  EXPECT_TRUE(sk_isxdigit('a'));
  EXPECT_TRUE(sk_isxdigit('f'));
  EXPECT_TRUE(sk_isxdigit('A'));
  EXPECT_TRUE(sk_isxdigit('F'));
  EXPECT_FALSE(sk_isxdigit('g'));
  EXPECT_FALSE(sk_isxdigit('G'));
}

TEST(SkCtypeTest, ToLower) {
  EXPECT_EQ(sk_tolower('A'), 'a');
  EXPECT_EQ(sk_tolower('Z'), 'z');
  EXPECT_EQ(sk_tolower('a'), 'a');
  EXPECT_EQ(sk_tolower('0'), '0');
}

TEST(SkCtypeTest, ToUpper) {
  EXPECT_EQ(sk_toupper('a'), 'A');
  EXPECT_EQ(sk_toupper('z'), 'Z');
  EXPECT_EQ(sk_toupper('A'), 'A');
  EXPECT_EQ(sk_toupper('0'), '0');
}

// 边界条件测试
TEST(SkCtypeTest, BoundaryValues) {
  // ASCII 边界值
  EXPECT_FALSE(sk_isalnum(-1));
  EXPECT_FALSE(sk_isalnum(128));
  EXPECT_FALSE(sk_isalpha(127));
}

TEST(SkCtypeTest, ToLowerBoundary) {
  // 边界值
  EXPECT_EQ(sk_tolower('A' - 1), 'A' - 1);  // '@'
  EXPECT_EQ(sk_tolower('Z' + 1), 'Z' + 1);  // '['
}

TEST(SkCtypeTest, ToUpperBoundary) {
  // 边界值
  EXPECT_EQ(sk_toupper('a' - 1), 'a' - 1);  // '`'
  EXPECT_EQ(sk_toupper('z' + 1), 'z' + 1);  // '{'
}

TEST(SkCtypeTest, AllDigits) {
  for (char c = '0'; c <= '9'; ++c) {
    EXPECT_TRUE(sk_isdigit(c));
    EXPECT_TRUE(sk_isalnum(c));
    EXPECT_TRUE(sk_isxdigit(c));
  }
}

TEST(SkCtypeTest, AllLetters) {
  for (char c = 'a'; c <= 'z'; ++c) {
    EXPECT_TRUE(sk_isalpha(c));
    EXPECT_TRUE(sk_isalnum(c));
    EXPECT_TRUE(sk_islower(c));
    EXPECT_FALSE(sk_isupper(c));
  }

  for (char c = 'A'; c <= 'Z'; ++c) {
    EXPECT_TRUE(sk_isalpha(c));
    EXPECT_TRUE(sk_isalnum(c));
    EXPECT_TRUE(sk_isupper(c));
    EXPECT_FALSE(sk_islower(c));
  }
}

TEST(SkCtypeTest, AllPunctuation) {
  // 测试常见标点符号
  const char punct[] = "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~";
  for (size_t i = 0; i < strlen(punct); ++i) {
    EXPECT_TRUE(sk_ispunct(punct[i]));
    EXPECT_FALSE(sk_isalnum(punct[i]));
  }
}

TEST(SkCtypeTest, AllWhitespace) {
  EXPECT_TRUE(sk_isspace(' '));   // space
  EXPECT_TRUE(sk_isspace('\t'));  // tab
  EXPECT_TRUE(sk_isspace('\n'));  // newline
  EXPECT_TRUE(sk_isspace('\r'));  // carriage return
  EXPECT_TRUE(sk_isspace('\f'));  // form feed
  EXPECT_TRUE(sk_isspace('\v'));  // vertical tab
}

TEST(SkCtypeTest, GraphPrintable) {
  // isgraph: 可打印但不包括空格
  // isprint: 可打印包括空格
  EXPECT_FALSE(sk_isgraph(' '));
  EXPECT_TRUE(sk_isprint(' '));

  EXPECT_TRUE(sk_isgraph('A'));
  EXPECT_TRUE(sk_isprint('A'));
}

TEST(SkCtypeTest, ControlCharacters) {
  // 测试控制字符 (0-31 和 127)
  for (int c = 0; c < 32; ++c) {
    EXPECT_TRUE(sk_iscntrl(c));
    EXPECT_FALSE(sk_isprint(c));
  }
  EXPECT_TRUE(sk_iscntrl(127));
}

TEST(SkCtypeTest, CaseConversion) {
  // 测试整个字母表的大小写转换
  for (char c = 'A'; c <= 'Z'; ++c) {
    EXPECT_EQ(sk_tolower(c), c + ('a' - 'A'));
  }

  for (char c = 'a'; c <= 'z'; ++c) {
    EXPECT_EQ(sk_toupper(c), c - ('a' - 'A'));
  }
}

TEST(SkCtypeTest, NonAscii) {
  // 测试非 ASCII 字符（假设函数处理扩展 ASCII）
  // 这些应该返回 false 对于大多数函数
  EXPECT_FALSE(sk_isalpha(128));
  EXPECT_FALSE(sk_isdigit(200));
  EXPECT_FALSE(sk_isalnum(255));
}
