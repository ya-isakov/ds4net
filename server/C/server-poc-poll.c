//#include <ev.h>
#include <stdio.h>
#include <sys/time.h>
#include <sys/fcntl.h>
#include <sys/socket.h>
#include <arpa/inet.h>
#include <string.h>
#include <poll.h>
#include <sys/timerfd.h>


int events = 0;
int problems = 0;
int client;
int server;

static void process_hidraw (int fd) {
    unsigned char buf[49];
    int nr=read(fd, buf, sizeof(buf));
    if ( nr < 0 ) {
            perror("read(stdin)");
            return;
    }
    int i=0;
    int s = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    //for (i; i++; i<3) {
    if (client) send(client, buf, sizeof(buf), 0);
    //}
    //events++;
    //printf("%d %02X %02X %02X\n", nr, buf[0], buf[1], buf[2]);
}

static void process_udp(fd) {
    puts("udp socket has become readable");
    char buf[6];
    struct sockaddr_in addr;
    int addr_len = sizeof(addr);
    socklen_t bytes = recvfrom(fd, buf, sizeof(buf) - 1, 0, (struct sockaddr*) &addr, &addr_len);
    printf("Got %d bytes\n", (int)bytes);
    switch (buf[1]) {
       case 0x00:
          printf("Connected client %s\n", inet_ntoa(addr.sin_addr));
          buf[2] = 0x02; // Connected
          buf[3] = 0x00; // Disconnected
          buf[4] = 0x00; // Disconnected
          buf[5] = 0x00; // Disconnected
          sendto(fd, buf, 6, 0, (struct sockaddr *)&addr, addr_len);
          addr.sin_port = htons(8888);
          int clientfd = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
          connect(clientfd, (struct sockaddr *)&addr, addr_len);
          client = clientfd;
          //printf("Got new client: %d\n", addr.);
          break;
       case 0x01:
          //syslog(LOG_INFO, "Rumble gamepad N%d: 0x%02x, 0x%02x", buf[0], buf[2], buf[3]);
          break;
       case 0x02:
          //syslog(LOG_INFO, "Unknown, gamepad N%d: 0x%02x, 0x%02x", buf[0], buf[2], buf[3]);
          break;
       case 0x03:
          //syslog(LOG_INFO, "Get global config");
          break;
       case 0x04:
          //syslog(LOG_INFO, "Set global config");
          break;
    }
}

int main (void) {
    int timerfd = timerfd_create(CLOCK_REALTIME, 0);
    struct itimerspec new_value;
    new_value.it_interval.tv_sec = 10;
    new_value.it_value.tv_sec = 10;
    timerfd_settime(timerfd, 0, &new_value, NULL);
    struct pollfd p[3];
    p[0].events = POLLIN;
    p[1].events = POLLIN;
    p[2].events = POLLIN;
    struct sockaddr_in sa;
    //socklen_t len = sizeof(sa);
    server = socket(AF_INET, SOCK_DGRAM, 0);
    memset(&sa, 0, sizeof(sa));
    sa.sin_family = AF_INET;
    sa.sin_port = htons(8888);
    sa.sin_addr.s_addr = INADDR_ANY;
    if (bind(server, (struct sockaddr*) &sa, sizeof(sa)) != 0) printf("error z\n");
    int hidraw_fd = open("/dev/hidraw0", O_RDWR | O_NONBLOCK);
    if(hidraw_fd < 0) {
        puts ("cannot open hidraw device");
        return 1;
    }
    p[0].fd = hidraw_fd;
    p[1].fd = server;
    p[2].fd = timerfd;
    uint64_t exp;
    while(1){
	int res = poll(p, 3, 1000);
	if (res == 0) continue;
	if (p[1].revents & POLLIN) {process_udp(p[1].fd);p[1].revents = 0;};
	if (p[0].revents & POLLIN) {process_hidraw(p[0].fd); p[0].revents = 0; problems++;};
        if (p[2].revents & POLLIN) {read(timerfd, &exp, sizeof(exp));  printf("%f\n", problems*1.0/10); problems=0;};
    }
    return 0;
}
