#include "fixture.hpp"

namespace {
constexpr u64 constructed_marker = 2;
constexpr u64 finalized_marker = 3;
constexpr u64 destroyed_marker = 0x44455354524f5944ULL;

volatile u64 phase;
volatile u64 destroyed;

struct GlobalEarly final {
    GlobalEarly() noexcept { phase = 1; }
    ~GlobalEarly() { destroyed = phase == finalized_marker ? destroyed_marker : 0; }
};

GlobalEarly lifecycle __attribute__((init_priority(200)));
}

extern "C" void woven_global_late_constructed() noexcept {
    phase = phase == 1 ? constructed_marker : 0;
}

extern "C" void woven_global_late_destroyed() noexcept {
    phase = phase == constructed_marker ? finalized_marker : 0;
}

extern "C" u64 woven_global_constructed() noexcept { return phase; }
extern "C" u64 woven_global_destroyed() noexcept { return destroyed; }
extern "C" u64 woven_global_constructed_marker() noexcept { return constructed_marker; }
extern "C" u64 woven_global_finalized_marker() noexcept { return finalized_marker; }
extern "C" u64 woven_global_destroyed_marker() noexcept { return destroyed_marker; }
