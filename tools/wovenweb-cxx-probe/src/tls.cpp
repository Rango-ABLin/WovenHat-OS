#include "fixture.hpp"
namespace {
alignas(64) thread_local volatile u64 word = 0x1122334455667788ULL;
alignas(64) thread_local volatile u64 zero_word;
}
extern "C" u64 woven_tls_word() noexcept { return word; }
extern "C" u64 woven_tls_zero() noexcept { return zero_word; }
extern "C" void woven_tls_store(u64 first, u64 second) noexcept { word = first; zero_word = second; }
extern "C" u64 woven_tls_address() noexcept { return reinterpret_cast<u64>(&word); }
