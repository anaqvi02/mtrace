#include <stdio.h>
#include <sys/stat.h>
#include <sys/time.h>
#include <unistd.h>

int main() {
    struct stat buf;
    struct timeval start, end;
    
    gettimeofday(&start, NULL);
    for(int i = 0; i < 500000; i++) {
        fstat(0, &buf);
    }
    gettimeofday(&end, NULL);
    
    long mtime, seconds, useconds;
    seconds  = end.tv_sec  - start.tv_sec;
    useconds = end.tv_usec - start.tv_usec;
    mtime = ((seconds) * 1000000 + useconds);
    
    printf("Total time: %ld us\n", mtime);
    printf("Time per call: %f ns\n", (float)mtime * 1000.0 / 500000.0);
    return 0;
}
