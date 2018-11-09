import socket
sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM) # UDP
sock.sendto(bytearray([0]), ("127.0.0.1", 9999))

import time
t = time.time()
count = 0
while True:
    z = sock.recv(80)
    print(z[7])
    count = count+1
    if time.time() - t >= 10:
        print(count/10)
        count = 0
        t = time.time()
