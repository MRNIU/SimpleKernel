
// This file is a part of Simple-XX/SimpleKernel
// (https://github.com/Simple-XX/SimpleKernel).
//
// vmm.cpp for Simple-XX/SimpleKernel.

#include "stdint.h"
#include "string.h"
#include "cpu.hpp"
#include "memlayout.h"
#include "vmm.h"

// class page_table_entry_t {
// public:
//     uint64_t v : 1;
//     uint64_t r : 1;
//     uint64_t w : 1;
//     uint64_t x : 1;
//     uint64_t u : 1;
//     uint64_t g : 1;
//     uint64_t a : 1;
//     uint64_t d : 1;
//     uint64_t rsw : 2;
//     uint64_t ppn0 : 9;
//     uint64_t ppn1 : 9;
//     uint64_t ppn2 : 26;
//     uint64_t reserved : 10;
// };

// 获取一个地址的 63−39，9 位
// 63−39 位的值必须等于第 38
// 位的值，否则会认为该虚拟地址不合法，在访问时会产生异常。
#define VMM_VPN_RESERVED(x) (((x) >> 39) & 0x3FFFFFF)

// PTE idx 的位数
static constexpr const uint32_t PTE_BITS = 12;
// PTE idx 的偏移
static constexpr const uint32_t PTE_SHIFT = 0;
static constexpr const uint32_t PTE_MASK  = 0xFFF;
// PMD idx 的位数
static constexpr const uint32_t PMD_BITS = 9;
// PMD idx 的偏移
static constexpr const uint32_t PMD_SHIFT = 12;
static constexpr const uint32_t PMD_MASK  = 0x1FF;
// PUD idx 的位数
static constexpr const uint32_t PUD_BITS = 9;
// PUD idx 的偏移
static constexpr const uint32_t PUD_SHIFT = 21;
static constexpr const uint32_t PUD_MASK  = 0x1FF;
// PGD idx 的位数
static constexpr const uint32_t PGD_BITS = 9;
// PGD idx 的偏移
static constexpr const uint32_t PGD_SHIFT = 30;
static constexpr const uint32_t PGD_MASK  = 0x1FF;

static constexpr uint32_t GET_PTE(uint32_t addr) {
    return (addr >> PTE_SHIFT) & PTE_MASK;
}

static constexpr uint32_t GET_PMD(uint32_t addr) {
    return (addr >> PMD_SHIFT) & PMD_MASK;
}

static constexpr uint32_t GET_PUD(uint32_t addr) {
    return (addr >> PUD_SHIFT) & PUD_MASK;
}

static constexpr uint32_t GET_PGD(uint32_t addr) {
    return (addr >> PGD_SHIFT) & PGD_MASK;
}

IO         VMM::io;
PMM        VMM::pmm;
page_dir_t VMM::pgd_kernel[VMM_PAGE_TABLES_TOTAL];

VMM::VMM(void) {
    curr_dir = (page_dir_t)CPU::READ_SATP();
    return;
}

VMM::~VMM(void) {
    return;
}

void VMM::init(void) {
    // 映射物理地址前 32MB 到虚拟地址前 32MB
    for (uint64_t addr = 0x80000000; addr < 0x84000000;
         // for (uint64_t addr = 0x80000000; addr < 0x81000000;
         addr += COMMON::PAGE_SIZE) {
        mmap((page_dir_t)pgd_kernel, (void *)addr, (void *)addr,
             VMM_PAGE_READABLE | VMM_PAGE_EXECUTABLE);
    }
    // mmap((page_dir_t)pgd_kernel, (void *)0x80000000, (void *)0x80000000,
    //      VMM_PAGE_READABLE | VMM_PAGE_EXECUTABLE);
    // mmap((page_dir_t)pgd_kernel, (void *)0x80001000, (void *)0x80001000,
    //      VMM_PAGE_READABLE | VMM_PAGE_EXECUTABLE);
    // mmap((page_dir_t)pgd_kernel, (void *)0x80002000, (void *)0x80002000,
    //      VMM_PAGE_READABLE | VMM_PAGE_EXECUTABLE);
    // mmap((page_dir_t)pgd_kernel, (void *)0x80003000, (void *)0x80003000,
    //      VMM_PAGE_READABLE | VMM_PAGE_EXECUTABLE);
    // mmap((page_dir_t)pgd_kernel, (void *)0x81000000, (void *)0x81000000,
    //      VMM_PAGE_READABLE | VMM_PAGE_EXECUTABLE);
    // mmap((page_dir_t)pgd_kernel, (void *)0x81001000, (void *)0x81001000,
    //      VMM_PAGE_READABLE | VMM_PAGE_EXECUTABLE);
    // mmap((page_dir_t)pgd_kernel, (void *)0x84000000, (void *)0x84000000,
    //      VMM_PAGE_READABLE | VMM_PAGE_EXECUTABLE);
    // 虚拟地址 0x00 设为 nullptr
    // unmmap((page_dir_t)pgd_kernel, nullptr);
    // set_pgd((page_dir_t)pgd_kernel);
    // CPU::SFENCE_VMA();
    io.printf("vmm_init\n");
    return;
}

page_dir_t VMM::get_pgd(void) const {
    return (page_dir_t)CPU::MAKE_SATP(curr_dir);
}

void VMM::set_pgd(const page_dir_t pgd) {
    curr_dir = pgd;
    CPU::WRITE_SATP(CPU::MAKE_SATP(curr_dir));
    return;
}

static uint32_t k = 0;
void            VMM::mmap(const page_dir_t pgd, const void *va, const void *pa,
               const uint32_t flag) {

    uint64_t pgd_idx = GET_PGD(reinterpret_cast<uint64_t>(va));
    uint64_t pud_idx = GET_PUD(reinterpret_cast<uint64_t>(va));
    uint64_t pmd_idx = GET_PMD(reinterpret_cast<uint64_t>(va));
    uint64_t pte_idx = GET_PTE(reinterpret_cast<uint64_t>(va));
#define DEBUG
#ifdef DEBUG
    io.info(
        "pgd_idx: 0x%X, pud_idx: 0x%X, pmd_idx: 0x%X, pte_idx: 0x%X, k: %d\n",
        pgd_idx, pud_idx, pmd_idx, pte_idx, k);
#undef DEBUG
#endif
    page_table_t pud =
        (page_table_t)((uint64_t)pgd[pgd_idx] & COMMON::PAGE_MASK);
    if (pud == nullptr) {
        pud = (page_table_t)pmm.alloc_page(1, COMMON::NORMAL);
        k++;
        io.info("pud alloc\n");
        pgd[pgd_idx] = (page_dir_entry_t)(reinterpret_cast<uint64_t>(pud) |
                                          VMM_PAGE_VALID);
    }

    page_table_t pmd =
        (page_table_t)((uint64_t)pud[pud_idx] & COMMON::PAGE_MASK);
    if (pmd == nullptr) {
        pmd = (page_table_t)pmm.alloc_page(1, COMMON::NORMAL);
        k++;
        io.info("pmd alloc\n");
        pud[pud_idx] = (page_dir_entry_t)(reinterpret_cast<uint64_t>(pmd) |
                                          VMM_PAGE_VALID);
    }
    page_table_t pt =
        (page_table_t)((uint64_t)pmd[pmd_idx] & COMMON::PAGE_MASK);
#define DEBUG
#ifdef DEBUG
    io.info("pt: 0x%X, pmd[%d]: 0x%X, pmd: 0x%X\n", pt, pmd_idx, pmd[pmd_idx],
            pmd);
#undef DEBUG
#endif
    if (pt == nullptr) {
        pt = (page_table_t)pmm.alloc_page(1, COMMON::NORMAL);
        // k++;
        // io.info("pt alloc\n");
        pmd[pmd_idx] =
            (page_dir_entry_t)(reinterpret_cast<uint64_t>(pt) | VMM_PAGE_VALID);
    }
// #define DEBUG
#ifdef DEBUG
    io.info("pt: 0x%X, pmd[%d]: 0x%X\n", pt, pmd_idx, pmd[pmd_idx]);
#undef DEBUG
#endif
    if (pa == nullptr) {
        pa = pmm.alloc_page(1, COMMON::HIGH);
    }
// #define DEBUG
#ifdef DEBUG
    io.info("pt: 0x%X, pa: 0x%X\n", pt, pa);
#undef DEBUG
#endif
    pt[pte_idx] = (page_table_entry_t)(
        (reinterpret_cast<uint64_t>(pa) & COMMON::PAGE_MASK) | flag);
// #define DEBUG
#ifdef DEBUG
    io.info("pt[pte_idx]: 0x%X, &: 0x%X, pa: 0x%X\n", pt[pte_idx], &pt[pte_idx],
            pa);
#undef DEBUG
#endif
    return;
}

void VMM::unmmap(const page_dir_t pgd, const void *va) {
    uint64_t     pgd_idx = GET_PGD(reinterpret_cast<uint64_t>(va));
    uint64_t     pte_idx = GET_PTE(reinterpret_cast<uint64_t>(va));
    page_table_t pt =
        (page_table_t)((uint64_t)pgd[pgd_idx] & COMMON::PAGE_MASK);
    if (pt == nullptr) {
        io.printf("pt == nullptr\n");
        return;
    }
    pt          = (page_table_t)VMM_PA_LA(pt);
    pt[pte_idx] = nullptr;
    // TODO: 如果一页表都被 unmap，释放占用的物理内存
    return;
}

uint32_t VMM::get_mmap(const page_dir_t pgd, const void *va, const void *pa) {
    uint64_t     pgd_idx = GET_PGD(reinterpret_cast<uint64_t>(va));
    uint64_t     pte_idx = GET_PTE(reinterpret_cast<uint64_t>(va));
    page_table_t pt =
        (page_table_t)((uint64_t)pgd[pgd_idx] & COMMON::PAGE_MASK);
    if (pt == nullptr) {
        if (pa != nullptr) {
            *(uint64_t *)pa = (uint64_t) nullptr;
        }
        return 0;
    }
    pt = (page_table_t)(VMM_PA_LA(pt));
    if (pt[pte_idx] != nullptr) {
        if (pa != nullptr) {
            *(uint64_t *)pa = (uint64_t)pt[pte_idx] & COMMON::PAGE_MASK;
        }
        return 1;
    }
    else {
        if (pa != nullptr) {
            *(uint64_t *)pa = (uint64_t) nullptr;
        }
        return 0;
    }
}