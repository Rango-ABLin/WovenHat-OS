#include "fixture.hpp"

namespace {
constexpr u64 constructed_marker = 0x434f4e5354525543ULL;
constexpr u64 destroyed_marker = 0x44455354524f5944ULL;

volatile u64 constructed;
volatile u64 destroyed;

struct GlobalLifecycle final {
    GlobalLifecycle() noexcept { constructed = constructed_marker; }
    ~GlobalLifecycle() { destroyed = constructed == constructed_marker ? destroyed_marker : 0; }
};

GlobalLifecycle lifecycle;
}

extern "C" u64 woven_global_constructed() noexcept { return constructed; }
extern "C" u64 woven_global_destroyed() noexcept { return destroyed; }
extern "C" u64 woven_global_constructed_marker() noexcept { return constructed_marker; }
extern "C" u64 woven_global_destroyed_marker() noexcept { return destroyed_marker; }
