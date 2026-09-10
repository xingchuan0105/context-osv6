// Minimal reproducer for the Rust/MinGW dynamic libstdc++ call_once crash.
#include <mutex>
extern "C" int once_probe() {
    static std::once_flag flag;
    static int count = 0;
    std::call_once(flag, [] { ++count; });
    return count;
}
