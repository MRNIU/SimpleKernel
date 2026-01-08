/**
 * @copyright Copyright The SimpleKernel Contributors
 */

#include <cpu_io.h>

#include <cstdint>

#include "arch.h"
#include "basic_info.hpp"
#include "kernel.h"
#include "sk_cstdio"
#include "sk_cstring"
#include "sk_libcxx.h"
#include "system_test.h"

template <uint32_t V>
class TestStaticConstructDestruct {
 public:
  explicit TestStaticConstructDestruct(unsigned int& v) : _v(v) { _v |= V; }
  ~TestStaticConstructDestruct() { _v &= ~V; }

 private:
  unsigned int& _v;
};

static int global_value_with_init = 42;
static uint32_t global_u32_value_with_init{0xa1a2a3a4UL};
static uint64_t global_u64_value_with_init{0xb1b2b3b4b5b6b7b8ULL};
static uint16_t global_u16_value_with_init{0x1234};
static uint8_t global_u8a_value_with_init{0x42};
static uint8_t global_u8b_value_with_init{0x43};
static uint8_t global_u8c_value_with_init{0x44};
static uint8_t global_u8d_value_with_init{0x45};
static volatile bool global_bool_keep_running{true};

static unsigned int global_value1_with_constructor = 1;
static unsigned int global_value2_with_constructor = 2;

static TestStaticConstructDestruct<0x200> constructor_destructor_1(
    global_value1_with_constructor);
static TestStaticConstructDestruct<0x200> constructor_destructor_2(
    global_value2_with_constructor);
static TestStaticConstructDestruct<0x100000> constructor_destructor_3{
    global_value2_with_constructor};
static TestStaticConstructDestruct<0x100000> constructor_destructor_4{
    global_value1_with_constructor};

class AbsClass {
 public:
  AbsClass() { val = 'B'; }
  virtual ~AbsClass() { ; }
  virtual void Func() = 0;
  char val = 'A';
};

class InsClass : public AbsClass {
 public:
  void Func() override { val = 'C'; }
};

auto ctor_dtor_test() -> bool {
#ifdef __aarch64__
  cpu_io::SetupFpu();
#endif

  global_u8c_value_with_init++;
  global_u32_value_with_init++;
  global_u64_value_with_init++;
  global_u8b_value_with_init++;
  global_u8d_value_with_init++;
  global_u16_value_with_init++;
  global_u8a_value_with_init++;
  global_value_with_init++;
  global_bool_keep_running = false;

  sk_putchar('1', nullptr);
  auto inst_class = InsClass();
  sk_putchar('2', nullptr);
  sk_printf("%c\n", inst_class.val);
  sk_putchar('3', nullptr);
  inst_class.Func();
  sk_putchar('4', nullptr);
  sk_printf("%c\n", inst_class.val);
  sk_putchar('5', nullptr);

  static InsClass inst_class_static;
  sk_printf("%c\n", inst_class_static.val);
  inst_class_static.Func();
  sk_printf("%c\n", inst_class_static.val);

  sk_printf("%ld\n", Singleton<BasicInfo>::GetInstance().elf_addr);

  sk_printf("Hello Test\n");

  return true;
}
