# Device Framework ETL Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace hand-rolled low-level C++ patterns in `src/device/` with ETL idioms — `etl::vector`, `etl::delegate`, `etl::flat_map`, `etl::optional` — to reduce boilerplate and improve clarity.

**Architecture:** Four self-contained changes applied in order: (1) container swap in DriverRegistry + DeviceManager, (2) delegate swap in DriverEntry + drivers, (3) DriverDescriptor elimination by merging its fields into DriverEntry, (4) O(N·M·K) FindDriver loop replaced with an etl::flat_map index. All framework headers remain header-only; no new `.cpp` files.

**Tech Stack:** C++23, ETL 20.x (`etl::vector`, `etl::flat_map`, `etl::delegate`, `etl::optional`, `etl::span`), `kstd::strcmp`/`strlen`, `Expected<T>` (std::expected alias), `LockGuard<SpinLock>`.

---

## Key ETL APIs (reference)

### etl::delegate — confirmed from `3rd/etl/include/etl/private/delegate_cpp11.h`

```cpp
// Free / static function
auto d = etl::delegate<bool(DeviceNode&)>::create<&Ns16550aDriver::MatchStatic>();

// Member function + runtime instance reference
auto d = etl::delegate<Expected<void>(DeviceNode&)>::create<
    Ns16550aDriver, &Ns16550aDriver::Probe>(Ns16550aDriver::Instance());

// Lambda / functor
auto d = etl::delegate<bool(DeviceNode&)>::create(my_lambda);
```

### etl::flat_map

```cpp
etl::flat_map<etl::string_view, size_t, N> m;
m.insert({etl::string_view("ns16550a"), 0});
auto it = m.find(etl::string_view("ns16550a"));   // O(log N)
if (it != m.end()) { size_t idx = it->second; }
```

### etl::optional

```cpp
etl::optional<T> opt;
opt.emplace(std::move(value));  // replaces Storage::Emplace
opt.has_value();                // replaces Storage::HasValue
opt.value();                    // replaces Storage::Value
opt.reset();                    // replaces Storage::Reset
```

### etl::vector

```cpp
etl::vector<DriverEntry, 32> v;
v.push_back(entry);
v.full();           // capacity check
v.size();
v.data();           // raw pointer for bus Enumerate
```

---

## Task 1: Refactor `driver_registry.hpp` — containers + flat_map index

**Files:**
- Modify: `src/device/include/driver_registry.hpp`

### Step 1: Replace the include block

Add ETL headers after existing includes:

```cpp
#include <etl/flat_map.h>
#include <etl/span.h>
#include <etl/string_view.h>
#include <etl/vector.h>
```

Remove `#include "kstd_cstring"` only if `kstd::strcmp`/`strlen` calls are removed from this file (they will be in Step 4). Keep it during Step 2–3.

### Step 2: Update `MatchEntry` — use `etl::string_view`

```cpp
/// One entry in a driver's static match table
struct MatchEntry {
  BusType          bus_type;
  etl::string_view compatible;  ///< FDT compatible string (.rodata ref, always valid)
};
```

> **Why safe:** `etl::string_view` stores a pointer + length. `kMatchTable` arrays in drivers are `static constexpr`, living in `.rodata` for the program's lifetime. No dangling pointer risk.

### Step 3: Replace raw function pointers in `DriverEntry` with `etl::delegate`

Also merge `DriverDescriptor` fields directly into `DriverEntry` (eliminating the indirection pointer `descriptor`):

```cpp
/**
 * @brief Type-erased driver entry — one per registered driver.
 *
 * @pre  match/probe/remove delegates are bound before registration
 */
struct DriverEntry {
  const char*                                          name;
  etl::span<const MatchEntry>                         match_table;
  etl::delegate<bool(DeviceNode&)>                    match;
  etl::delegate<Expected<void>(DeviceNode&)>          probe;
  etl::delegate<Expected<void>(DeviceNode&)>          remove;
};
```

Remove the old `DriverDescriptor` struct entirely.

### Step 4: Replace raw array + counter with `etl::vector`; add `flat_map` index

```cpp
class DriverRegistry {
 public:
  /**
   * @brief Register a driver entry.
   *
   * @pre  entry.match/probe/remove delegates are bound
   * @return Expected<void> kOutOfMemory if registry is full
   */
  auto Register(const DriverEntry& entry) -> Expected<void>;

  /**
   * @brief Find the first driver whose match_table has a compatible string
   *        present in node.compatible (flat_map lookup, O(Cn · log T)).
   *
   * @return pointer to DriverEntry, or nullptr if no match
   */
  auto FindDriver(const DeviceNode& node) -> const DriverEntry*;

  DriverRegistry() = default;
  ~DriverRegistry() = default;
  DriverRegistry(const DriverRegistry&) = delete;
  DriverRegistry(DriverRegistry&&) = delete;
  auto operator=(const DriverRegistry&) -> DriverRegistry& = delete;
  auto operator=(DriverRegistry&&) -> DriverRegistry& = delete;

 private:
  static constexpr size_t kMaxDrivers = 32;
  /// Max total MatchEntry rows across all drivers (32 drivers × ~3 compat strings)
  static constexpr size_t kMaxCompatEntries = 96;

  etl::vector<DriverEntry, kMaxDrivers>                          drivers_;
  etl::flat_map<etl::string_view, size_t, kMaxCompatEntries>    compat_map_;
  SpinLock lock_{"driver_registry"};
};
```

### Step 5: Rewrite `Register` inline implementation

```cpp
inline auto DriverRegistry::Register(const DriverEntry& entry) -> Expected<void> {
  LockGuard guard(lock_);
  if (drivers_.full()) {
    return std::unexpected(Error(ErrorCode::kOutOfMemory));
  }
  const size_t idx = drivers_.size();
  for (const auto& me : entry.match_table) {
    if (compat_map_.full()) {
      return std::unexpected(Error(ErrorCode::kOutOfMemory));
    }
    // insert() is a no-op on duplicate key; that is acceptable —
    // first-registered driver wins for a given compatible string.
    compat_map_.insert({me.compatible, idx});
  }
  drivers_.push_back(entry);
  return {};
}
```

### Step 6: Rewrite `FindDriver` inline implementation

```cpp
inline auto DriverRegistry::FindDriver(const DeviceNode& node)
    -> const DriverEntry* {
  const char* p   = node.compatible;
  const char* end = node.compatible + node.compatible_len;
  while (p < end) {
    const auto it = compat_map_.find(etl::string_view(p));
    if (it != compat_map_.end()) {
      auto& entry = drivers_[it->second];
      if (entry.match(const_cast<DeviceNode&>(node))) {
        return &entry;
      }
    }
    p += kstd::strlen(p) + 1;
  }
  return nullptr;
}
```

> **Note:** `kstd::strlen` is still used here for walking the null-terminated compatible stringlist. Keep `#include "kstd_cstring"`.

### Step 7: Verify build

```bash
cmake --preset build_riscv64
cd build_riscv64 && make SimpleKernel 2>&1 | head -50
```

Expected: compile errors in the two driver files (they still reference the old `DriverDescriptor` / raw function pointers). That is expected — fix them in Tasks 2 & 3.

### Step 8: Commit

```bash
git add src/device/include/driver_registry.hpp
git commit --signoff -m "refactor(device): replace raw array+ptr with etl::vector/flat_map in DriverRegistry"
```

---

## Task 2: Update `ns16550a_driver.hpp` — etl::delegate + remove DriverDescriptor

**Files:**
- Modify: `src/device/include/driver/ns16550a_driver.hpp`

### Step 1: Remove `DriverDescriptor`-related members

Delete these two lines from the `private:` section:
```cpp
static const DriverDescriptor kDescriptor;   // DELETE
```
And delete the out-of-class definition at the bottom:
```cpp
inline const DriverDescriptor Ns16550aDriver::kDescriptor{ ... };   // DELETE
```

### Step 2: Update `GetEntry()` to use `etl::delegate` and the new `DriverEntry` layout

Replace the entire `GetEntry()` body:

```cpp
static auto GetEntry() -> const DriverEntry& {
  static const DriverEntry entry{
      .name        = "ns16550a",
      .match_table = etl::span<const MatchEntry>(kMatchTable),
      .match       = etl::delegate<bool(DeviceNode&)>::create<
                         &Ns16550aDriver::MatchStatic>(),
      .probe       = etl::delegate<Expected<void>(DeviceNode&)>::create<
                         Ns16550aDriver, &Ns16550aDriver::Probe>(Instance()),
      .remove      = etl::delegate<Expected<void>(DeviceNode&)>::create<
                         Ns16550aDriver, &Ns16550aDriver::Remove>(Instance()),
  };
  return entry;
}
```

> **Note on `etl::span` construction:** `etl::span<const MatchEntry>(kMatchTable)` deduces size from the array. This is equivalent to `std::span(kMatchTable)`.

### Step 3: Update the include block

Add:
```cpp
#include <etl/span.h>
```

Remove:
```cpp
// No new removals; driver_registry.hpp already pulls in etl headers transitively
```

### Step 4: Update ProbeAll in device_manager.hpp to use the new `name` field

`device_manager.hpp` currently references `drv->descriptor->name`. Change every occurrence to `drv->name`:

In `ProbeAll()`:
```cpp
// Before:
klog::Debug("DeviceManager: driver '%s' rejected '%s'\n",
            drv->descriptor->name, node.name);
// ...
klog::Info("DeviceManager: probing '%s' with driver '%s'\n", node.name,
           drv->descriptor->name);
// ...
klog::Info("DeviceManager: '%s' bound to '%s'\n", node.name,
           drv->descriptor->name);

// After (replace descriptor->name with name in all three locations):
klog::Debug("DeviceManager: driver '%s' rejected '%s'\n",
            drv->name, node.name);
// ...
klog::Info("DeviceManager: probing '%s' with driver '%s'\n", node.name,
           drv->name);
// ...
klog::Info("DeviceManager: '%s' bound to '%s'\n", node.name,
           drv->name);
```

### Step 5: Verify build

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | head -50
```

Expected: compile errors only from `virtio_blk_driver.hpp`. Fix in Task 3.

### Step 6: Commit

```bash
git add src/device/include/driver/ns16550a_driver.hpp \
        src/device/include/device_manager.hpp
git commit --signoff -m "refactor(device): migrate Ns16550aDriver to etl::delegate, remove DriverDescriptor"
```

---

## Task 3: Update `virtio_blk_driver.hpp` — etl::delegate + etl::optional

**Files:**
- Modify: `src/device/include/driver/virtio_blk_driver.hpp`

### Step 1: Replace `detail::Storage<VirtioBlkType>` with `etl::optional<VirtioBlkType>`

In the `private:` section change:
```cpp
detail::Storage<VirtioBlkType> device_;   // DELETE
```
to:
```cpp
etl::optional<VirtioBlkType> device_;
```

### Step 2: Update all call sites inside `virtio_blk_driver.hpp`

| Old | New |
|-----|-----|
| `device_.Emplace(std::move(*result))` | `device_.emplace(std::move(*result))` |
| `device_.Value().GetCapacity()` | `device_.value().GetCapacity()` |
| `device_.Value().HandleInterrupt(...)` | `device_.value().HandleInterrupt(...)` |
| `device_.HasValue()` | `device_.has_value()` |
| `device_.Value()` (in `GetDevice`) | `device_.value()` |
| `device_.Reset()` | `device_.reset()` |

**`Probe()`** (line 125): `device_.Emplace(std::move(*result));` → `device_.emplace(std::move(*result));`

**`Probe()` log line** (line 132): `device_.Value().GetCapacity()` → `device_.value().GetCapacity()`

**`Remove()`** (line 138): `device_.Reset();` → `device_.reset();`

**`GetDevice()`** (line 143–145):
```cpp
[[nodiscard]] auto GetDevice() -> VirtioBlkType* {
  return device_.has_value() ? &device_.value() : nullptr;
}
```

**`HandleInterrupt()`** (lines 151–154):
```cpp
if (device_.has_value()) {
  device_.value().HandleInterrupt(
      static_cast<CompletionCallback&&>(on_complete));
}
```

### Step 3: Remove `#include "driver/detail/storage.hpp"`

Replace with:
```cpp
#include <etl/optional.h>
```

### Step 4: Update `GetEntry()` to use `etl::delegate` and new `DriverEntry` layout

Replace the entire `GetEntry()` body:

```cpp
static auto GetEntry() -> const DriverEntry& {
  static const DriverEntry entry{
      .name        = "virtio-blk",
      .match_table = etl::span<const MatchEntry>(kMatchTable),
      .match       = etl::delegate<bool(DeviceNode&)>::create<
                         &VirtioBlkDriver::MatchStatic>(),
      .probe       = etl::delegate<Expected<void>(DeviceNode&)>::create<
                         VirtioBlkDriver, &VirtioBlkDriver::Probe>(Instance()),
      .remove      = etl::delegate<Expected<void>(DeviceNode&)>::create<
                         VirtioBlkDriver, &VirtioBlkDriver::Remove>(Instance()),
  };
  return entry;
}
```

### Step 5: Remove the old `DriverDescriptor` out-of-class definition

Delete:
```cpp
inline const DriverDescriptor VirtioBlkDriver::kDescriptor{ ... };
```

And remove from `private:`:
```cpp
static const DriverDescriptor kDescriptor;   // DELETE
```

### Step 6: Add `etl/span.h` include; remove `kDescriptor` forward declaration

```cpp
#include <etl/optional.h>
#include <etl/span.h>
```

### Step 7: Full build verification

```bash
cd build_riscv64 && make SimpleKernel 2>&1 | grep -E "error:|warning:" | head -30
```

Expected: clean build (zero errors).

### Step 8: Run unit tests

```bash
cd build_riscv64 && make unit-test 2>&1 | tail -20
```

Expected: all tests pass (or pre-existing failures only — note any that were failing before).

### Step 9: Commit

```bash
git add src/device/include/driver/virtio_blk_driver.hpp
git commit --signoff -m "refactor(device): migrate VirtioBlkDriver to etl::optional + etl::delegate"
```

---

## Task 4: Delete `detail/storage.hpp` (now dead code)

**Files:**
- Delete: `src/device/include/driver/detail/storage.hpp`

### Step 1: Confirm no remaining includes

```bash
grep -r "storage.hpp" src/
```

Expected output: empty (zero matches). If any file still includes it, fix that file first.

### Step 2: Delete the file

```bash
git rm src/device/include/driver/detail/storage.hpp
```

### Step 3: Final build + test

```bash
cd build_riscv64 && make SimpleKernel && make unit-test 2>&1 | tail -20
```

Expected: clean build, all tests pass.

### Step 4: Commit

```bash
git add -A
git commit --signoff -m "refactor(device): delete detail/storage.hpp, superseded by etl::optional"
```

---

## Task 5: Run pre-commit and fix any formatting issues

### Step 1: Run formatter

```bash
cd /home/nzh/MRNIU/SimpleKernel && pre-commit run --all-files 2>&1 | head -60
```

### Step 2: If clang-format reports diffs, stage and amend

```bash
git add src/device/include/
git commit --signoff --amend --no-edit
```

---

## Completion Checklist

- [ ] `DriverDescriptor` struct is gone — no references remain
- [ ] `detail/storage.hpp` is deleted — no references remain
- [ ] `DriverEntry` has `etl::delegate` for match/probe/remove
- [ ] `DriverRegistry` uses `etl::vector<DriverEntry,32>` + `etl::flat_map<etl::string_view,size_t,96>`
- [ ] `FindDriver` uses `compat_map_.find()` instead of triple loop
- [ ] `DeviceManager` references `drv->name` (not `drv->descriptor->name`)
- [ ] `VirtioBlkDriver::device_` is `etl::optional<VirtioBlkType>`
- [ ] `make SimpleKernel` succeeds (zero errors)
- [ ] `make unit-test` passes
- [ ] `pre-commit run --all-files` clean
