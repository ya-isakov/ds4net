#include <ev.h>
#include <stdio.h>
#include <sys/time.h>
#include <sys/fcntl.h>
#include <sys/socket.h>
#include <arpa/inet.h>
#include <string.h>

ev_timer timeout_watcher, timeout_watcher2;
ev_io hidraw_watcher;
ev_io udp_watcher;
int events = 0;
int problems = 0;
int client;
int server;

static void timeout_cb (EV_P_ ev_timer *w, int revents) {
    struct timeval tv;
    gettimeofday(&tv, NULL);
    double ret = (double)(1000000.0*tv.tv_sec+tv.tv_usec);
    //printf ("timeout %lf\n", ret/1000000.0);
    double rps = events*1.0/2.5;
    if (problems == 0) {
	printf("bluetooth problem, rps = %f\n", rps);
	problems = 1;
    }
    events = 0;
}

static void hidraw_cb (EV_P_ struct ev_io *w, int revents) {
    unsigned char buf[96];
    int nr=read(w->fd, buf, sizeof(buf));
    if ( nr < 0 ) {
            perror("read(stdin)");
            return;
    }
    int i=0;
    //int s = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    //for (i; i++; i<3) {
    send(client, buf, sizeof(buf), 0);
    //}
    events++;
    //printf("%d %02X %02X %02X\n", nr, buf[0], buf[1], buf[2]);
}

static void udp_cb(EV_P_ ev_io *w, int revents) {
    puts("udp socket has become readable");
    char buf[6];
    struct sockaddr_in addr;
    int addr_len = sizeof(addr);
    socklen_t bytes = recvfrom(w->fd, buf, sizeof(buf) - 1, 0, (struct sockaddr*) &addr, &addr_len);
    printf("Got %d bytes\n", (int)bytes);
    switch (buf[1]) {
       case 0x00:
          printf("Connected client %s\n", inet_ntoa(addr.sin_addr));
          memset(&buf, 0, sizeof(buf))
          buf[2] = 0x02; // Connected
          buf[3] = 0x00; // Disconnected
          buf[4] = 0x00; // Disconnected
          buf[5] = 0x00; // Disconnected
          sendto(w->fd, buf, 6, 0, (struct sockaddr *)&addr, addr_len);
          //addr.sin_port = htons(26761);
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

static void timeout_cb_2 (EV_P_ ev_timer *w, int revents) {
    problems = 0;
}

int main (void) {
    struct sockaddr_in sa;
    //socklen_t len = sizeof(sa);
    server = socket(AF_INET, SOCK_DGRAM, 0);
    memset(&sa, 0, sizeof(sa));
    sa.sin_family = AF_INET;
    sa.sin_port = htons(26760);
    sa.sin_addr.s_addr = INADDR_ANY;
    if (bind(server, (struct sockaddr*) &sa, sizeof(sa)) != 0) printf("error z\n");
    int hidraw_fd = open("/dev/hidraw4", O_RDWR | O_NONBLOCK);
    if(hidraw_fd < 0) {
        puts ("cannot open hidraw device");
        return 1;
    }
    struct ev_loop *loop = ev_loop_new(EVBACKEND_POLL);
    ev_io_init(&udp_watcher, udp_cb, server, EV_READ);
    ev_io_start(loop, &udp_watcher);
    ev_io_init (&hidraw_watcher, hidraw_cb, hidraw_fd, EV_READ);
    ev_io_start (loop, &hidraw_watcher);
    ev_timer_init (&timeout_watcher, timeout_cb, 0., 2.5);
    ev_timer_start (loop, &timeout_watcher);
    ev_timer_init (&timeout_watcher2, timeout_cb_2, 0., 60.);
    ev_timer_start (loop, &timeout_watcher2);
    ev_run (loop, 0);
    close(hidraw_fd);
    close(client);
    close(server);
    return 0;
}
