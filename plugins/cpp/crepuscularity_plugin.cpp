#include <array>
#include <cstdio>
#include <cerrno>
#include <cstdlib>
#include <stdexcept>
#include <string>
#include <unistd.h>
#include <sys/wait.h>

static std::string exec_argv(const char *bin, const char *path, bool html = false) {
    int pipefd[2];
    if (pipe(pipefd) == -1) throw std::runtime_error("pipe failed");

    pid_t pid = fork();
    if (pid == -1) { close(pipefd[0]); close(pipefd[1]); throw std::runtime_error("fork failed"); }

    if (pid == 0) {
        close(pipefd[0]);
        dup2(pipefd[1], STDOUT_FILENO);
        close(pipefd[1]);
        // ponytail: argv exec, no shell
        if (html) {
            // An explicit empty context prevents sibling context.toml autoload.
            execlp(bin, bin, "render", path, "--ctx", "/dev/null", (char *)nullptr);
        } else {
            execlp(bin, bin, "native", "ir", path, (char *)nullptr);
        }
        _exit(127);
    }

    close(pipefd[1]);
    std::string result;
    char buf[4096];
    bool read_failed = false;
    for (;;) {
        ssize_t n = read(pipefd[0], buf, sizeof(buf));
        if (n < 0) {
            if (errno == EINTR) continue;
            read_failed = true;
            break;
        }
        if (n == 0) break;
        result.append(buf, static_cast<std::size_t>(n));
    }
    close(pipefd[0]);
    int status = 0;
    pid_t waited;
    do { waited = waitpid(pid, &status, 0); } while (waited == -1 && errno == EINTR);
    if (read_failed || waited != pid || !WIFEXITED(status) || WEXITSTATUS(status) != 0) {
        throw std::runtime_error(html ? "crepus render failed" : "crepus native ir failed");
    }
    return result;
}

struct ViewIr {
    int version;
    std::string json;
};

static std::string crepus_bin() {
    const char* env = std::getenv("CREPUS_BIN");
    return env != nullptr ? env : "crepus";
}

ViewIr render_ir(const std::string& path) {
    std::string bin = crepus_bin();
    std::string json = exec_argv(bin.c_str(), path.c_str());
    int version = json.find("\"version\":7") != std::string::npos || json.find("\"version\": 7") != std::string::npos ? 7 : -1;
    return {version, json};
}

std::string render_html(const std::string& path) {
    const std::string bin = crepus_bin();
    return exec_argv(bin.c_str(), path.c_str(), true);
}

int main(int argc, char** argv) {
    if (argc != 2) {
        return 2;
    }
    return render_ir(argv[1]).version == 7 && !render_html(argv[1]).empty() ? 0 : 1;
}
