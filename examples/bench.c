#include <stdio.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/time.h>

int main() {
    int fd = open("/dev/null", O_WRONLY);
    if (fd < 0) return 1;

    struct timeval start, end;
    gettimeofday(&start, NULL);

    for (int i = 0; i < 100000; i++) {
        write(fd, "a", 1);
    }

    gettimeofday(&end, NULL);
    close(fd);

    long seconds = end.tv_sec - start.tv_sec;
    long microseconds = end.tv_usec - start.tv_usec;
    double elapsed = seconds + microseconds * 1e-6;

    printf("100,000 writes to /dev/null took %.6f seconds\n", elapsed);
    return 0;
}
