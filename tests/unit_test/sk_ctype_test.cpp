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
