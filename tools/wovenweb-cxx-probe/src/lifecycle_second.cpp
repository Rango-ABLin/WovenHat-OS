#include "fixture.hpp"

extern "C" void woven_global_late_constructed() noexcept;
extern "C" void woven_global_late_destroyed() noexcept;

namespace {
struct GlobalLate final {
    GlobalLate() noexcept { woven_global_late_constructed(); }
    ~GlobalLate() { woven_global_late_destroyed(); }
};

GlobalLate lifecycle_second __attribute__((init_priority(300)));
}
