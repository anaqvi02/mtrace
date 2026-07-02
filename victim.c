#include <stdio.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/socket.h>

int main() {
    printf("Victim: I am going to open test.txt now...\n");
    int fd = open("test.txt", O_RDONLY);
    if (fd == -1) {
        printf("Victim: Failed to open file (this is expected if mactrace works!)\n");
    } else {
        printf("Victim: Success! FD is %d\n", fd);
        
        char buf[16];
        read(fd, buf, sizeof(buf));
        
        write(1, "Victim: directly writing to stdout\n", 35);
        
        close(fd);
    }
    
    printf("Victim: creating a dummy network socket...\n");
    int sock = socket(AF_INET, SOCK_STREAM, 0);
    if (sock != -1) {
        close(sock);
    }
    
    return 0;
}