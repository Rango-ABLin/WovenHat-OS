#pragma once
using usize = __SIZE_TYPE__;
using u64 = __UINT64_TYPE__;
extern "C" void* woven_alloc(usize, usize) noexcept;
extern "C" void woven_free(void*, usize, usize) noexcept;
extern "C" [[noreturn]] void woven_fail(u64) noexcept;
extern "C" u64 woven_mix(u64, u64, u64, u64, u64, u64, u64, u64) noexcept;
struct Pair { u64 first; u64 second; };
static_assert(sizeof(Pair) == 16 && __builtin_offsetof(Pair, second) == 8);
extern "C" Pair woven_pair(Pair) noexcept;
struct Base {
    virtual u64 value() const noexcept = 0;
    virtual ~Base() = default;
};
u64 dispatch(Base const*) noexcept;
