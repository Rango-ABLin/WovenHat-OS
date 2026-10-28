/*
 * Freestanding C ABI companion for the native C++ fixture.
 *
 * There are deliberately no headers, startup objects or libc calls here. This
 * object verifies C-to-Rust SysV calls, aggregate return layout, initialized
 * data and zero-initialized storage under the same target-qualified Clang
 * invocation as the C++ fixture.
 */

typedef unsigned long long wh_u64;

struct wh_pair {
    wh_u64 first;
    wh_u64 second;
};

extern wh_u64 woven_mix(
    wh_u64 first,
    wh_u64 second,
    wh_u64 third,
    wh_u64 fourth,
    wh_u64 fifth,
    wh_u64 sixth,
    wh_u64 seventh,
    wh_u64 eighth);
extern struct wh_pair woven_pair(struct wh_pair pair);

static wh_u64 c_initialized = 0x435f4142495f5631ULL;
static wh_u64 c_zero;

wh_u64 woven_c_abi_run(void) {
    struct wh_pair input = {2, 3};
    struct wh_pair output = woven_pair(input);
    if (woven_mix(1, 2, 3, 4, 5, 6, 7, 8) != 204) {
        return 70;
    }
    if (output.first != 5 || output.second != 6) {
        return 71;
    }
    if (c_initialized != 0x435f4142495f5631ULL || c_zero != 0) {
        return 72;
    }
    c_zero = output.first + output.second;
    return c_zero == 11 ? 0 : 73;
}
