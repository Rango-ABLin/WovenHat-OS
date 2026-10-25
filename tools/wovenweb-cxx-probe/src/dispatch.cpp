#include "fixture.hpp"
// Separate translation unit, without C++ LTO: exercise an actual virtual call.
__attribute__((noinline)) u64 dispatch(Base const* object) noexcept {
    return object->value();
}
