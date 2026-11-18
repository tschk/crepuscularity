#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/wait.h>
#include <errno.h>

static const char *crepus_bin(void) {
    const char *env = getenv("CREPUS_BIN");
    return env != NULL ? env : "crepus";
}

static int crepus_run_cli(const char *path, char *buf, size_t cap, int html) {
    if (buf == NULL || cap == 0) return 1;
    const char *bin = crepus_bin();
    int pipefd[2];
    if (pipe(pipefd) == -1) return 1;

    pid_t pid = fork();
    if (pid == -1) { close(pipefd[0]); close(pipefd[1]); return 1; }

    if (pid == 0) {
        close(pipefd[0]);
        dup2(pipefd[1], STDOUT_FILENO);
        close(pipefd[1]);
        // ponytail: argv exec, no shell
        if (html) {
            // An explicit empty context prevents sibling context.toml autoload.
            execlp(bin, bin, "render", path, "--ctx", "/dev/null", (char *)NULL);
        } else {
            execlp(bin, bin, "native", "ir", path, (char *)NULL);
        }
        _exit(127);
    }

    close(pipefd[1]);
    size_t used = 0;
    int failed = 0;
    char chunk[4096];
    for (;;) {
        ssize_t n = read(pipefd[0], chunk, sizeof(chunk));
        if (n < 0) {
            if (errno == EINTR) continue;
            failed = 1;
            break;
        }
        if (n == 0) break;
        size_t available = cap - 1 - used;
        size_t count = (size_t)n < available ? (size_t)n : available;
        memcpy(buf + used, chunk, count);
        used += count;
        if (count < (size_t)n) failed = 1;
    }
    close(pipefd[0]);
    int status;
    pid_t waited;
    do { waited = waitpid(pid, &status, 0); } while (waited == -1 && errno == EINTR);
    buf[used] = '\0';
    if (failed || waited != pid) return 1;
    return WIFEXITED(status) && WEXITSTATUS(status) == 0 ? 0 : 1;
}

static int crepus_render_ir(const char *path, char *buf, size_t cap) {
    return crepus_run_cli(path, buf, cap, 0);
}

static int crepus_render_html(const char *path, char *buf, size_t cap) {
    return crepus_run_cli(path, buf, cap, 1);
}

int main(int argc, char **argv) {
    if (argc != 2) {
        return 2;
    }
    char buf[8192];
    if (crepus_render_ir(argv[1], buf, sizeof(buf)) != 0) {
        return 1;
    }
    char html[8192];
    if (crepus_render_html(argv[1], html, sizeof(html)) != 0) {
        return 1;
    }
    return strstr(buf, "\"version\":7") || strstr(buf, "\"version\": 7") ? 0 : 1;
}
