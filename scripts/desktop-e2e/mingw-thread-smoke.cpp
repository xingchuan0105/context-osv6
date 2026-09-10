// Runs without providers or a desktop. Compile with the product's C++ compiler
// and run beside staged DLLs to catch missing exports and thread-model mismatch.
#include <cstdio>
#include <thread>
#include <mutex>
#include <condition_variable>

int main() {
    std::fprintf(stderr, "THREAD START\n");
    std::mutex lock;
    std::condition_variable ready;
    bool done = false;
    std::thread task([&] {
        std::lock_guard<std::mutex> guard(lock);
        done = true;
        ready.notify_one();
    });
    {
        std::unique_lock<std::mutex> guard(lock);
        ready.wait(guard, [&] { return done; });
    }
    task.join();
    std::fprintf(stderr, "PASS thread/mutex/condition_variable\n");
}
