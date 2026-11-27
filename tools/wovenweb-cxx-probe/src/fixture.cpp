#include "fixture.hpp"
static u64 constructed;
static u64 destroyed;
static u64 statics_initialized;
static constinit u64 data_value = 11;
struct alignas(64) Widget final : Base {
    u64 payload;
    explicit Widget(u64 value) noexcept : payload(value) { ++constructed; }
    ~Widget() override { ++destroyed; }
    u64 value() const noexcept override { return payload * 3 + 5; }
    static void* operator new(usize size) {
        void* result = woven_alloc(size, alignof(Widget));
        if (!result) woven_fail(20);
        return result;
    }
    static void operator delete(void* pointer) noexcept {
        woven_free(pointer, sizeof(Widget), alignof(Widget));
    }
};
struct Gate { Gate() noexcept { ++statics_initialized; } };
__attribute__((noinline)) static Gate& local_static() noexcept {
    static Gate instance;
    return instance;
}
// Unexpected use is a fixture failure; these are not a general allocation ABI.
void operator delete(void*) noexcept { woven_fail(21); }
void operator delete(void*, usize) noexcept { woven_fail(22); }
extern "C" u64 woven_cxx_run() noexcept {
    if (constructed || destroyed || statics_initialized || data_value != 11) return 1;
    if (woven_mix(1,2,3,4,5,6,7,8) != 204) return 2;
    Pair pair = woven_pair(Pair {3,7});
    if (pair.first != 10 || pair.second != 21) return 7;
    { Widget scoped(99); if (dispatch(&scoped) != 302) return 8; }
    Gate* first = &local_static();
    if (&local_static() != first || statics_initialized != 1) return 3;
    for (u64 i = 0; i < 512; ++i) {
        Base* object = new Widget(i);
        if (reinterpret_cast<usize>(object) % alignof(Widget) != 0) return 4;
        if (dispatch(object) != i * 3 + 5) return 5;
        delete object;
    }
    if (constructed != 513 || destroyed != 513) return 6;
    return 0;
}
