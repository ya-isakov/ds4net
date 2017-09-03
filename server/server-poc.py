import asyncio
from io import FileIO
import os
from select import epoll, EPOLLIN
import struct
import binascii
#import numpy
import matplotlib.pyplot as plt

prev = {"x": 0, "y": 0}

report_fd = os.open("/dev/hidraw0", os.O_RDWR | os.O_NONBLOCK)
tfd = FileIO(report_fd, "w+", closefd=False)

reports = 0
clients = {}

DEFAULT_LATENCY = 4


def connection(data, addr, transport):
    print("connected %s %s" % addr)
    clients[addr] = transport

def rumble(data, addr, transport):
    gamepad = data[1]
    left = data[2]
    right = data[3]
    print("Rumble from %s: gamepad %d, left %d, right %d" %(
          addr, gamepad, left, right))
    control(left, right)

def disconnect(data, addr, transport):
    print("Client %s %s disconnected" % addr)
    clients.pop(addr, None)

handlers = {
    0: connection,
    1: disconnect,
    2: rumble
}


class UDPProto:
    def connection_made(self, transport):
        self.transport = transport

    def datagram_received(self, data, addr):
        #message = data.decode()
        print('Received connection from %s %s' % addr)
        print('Send %r to %s' % (data, addr))
        handlers[data[0]](data, addr, self.transport)
        #self.transport.sendto(data, addr)

mapping = {
    0x1: "dpad_up",
    0x2: "dpad_down",
    0x4: "dpad_left",
    0x8: "dpad_right",
    0x10: "start",
    0x20: "back",
    0x40: "left_thumb",
    0x80: "right_thumb",
    0x100: "left_shoulder",
    0x200: "right_shoulder",
    0x1000: "a",
    0x2000: "b",
    0x4000: "x",
    0x8000: "y"
}

convert = {
    "start": "opt",
    "back": "share",
    "left_thumb": "l3",
    "right_thumb": "r3",
    "left_shoulder": "l1",
    "right_shoulder": "r1",
    "a": "cross",
    "b": "circle",
    "x": "square",
    "y": "triangle"
}

def control(big_rumble=0, small_rumble=0,
            led_red=0, led_green=0, led_blue=0x40):
    pkt = bytearray(74)
    pkt[0] = 0x11
    pkt[1] = 0xC0 | DEFAULT_LATENCY
    pkt[3] = 0x07
    offset = 3
    report_id = 0x11
    pkt[offset+3] = min(small_rumble, 255)
    pkt[offset+4] = min(big_rumble, 255)
    pkt[offset+5] = min(led_red, 255)
    pkt[offset+6] = min(led_green, 255)
    pkt[offset+7] = min(led_blue, 255)
    # Time to flash bright (255 = 2.5 seconds)
    pkt[offset+8] = 0 # min(flash_led1, 255)
    # Time to flash dark (255 = 2.5 seconds)
    pkt[offset+9] = 0 #min(flash_led2, 255)
    #pkt[73] = 0x7C
    #pkt[74] = 0x85
    #pkt[75] = 0xAB
    #pkt[76] = 0xC4
    #if self.type == "bluetooth":
    t = bytearray([0xA2])+pkt
    pkt = pkt+struct.pack("<L", binascii.crc32(t))
    tfd.write(pkt)
    #write_report(report_id, pkt)

def get_bit(data, num, bit):
    return (data[num] & (1 << bit)) !=0


def correct(v, min_, max_, dz):
    t = (max_ + min_) // 2
    c0 = t - dz
    c1 = t + dz
    t = (max_ - min_ - 4 * dz) // 2 
    c2 = (1 << 29) // t
    r0 = (c2 * (v - c0)) >> 14
    r1 = (c2 * (v - c1)) >> 14
    if v<c0:
        if (r0>32768 or r0<-32767):
            print(v, r0)
        return max(-32767, min(r0, 32767))
    elif v>c1:
        if (r1>32768 or r1<-32767):
            print(v, r1)
        return max(-32767, min(r1, 32767))
    return 0

def parse(hid):
    dpad = hid[5] % 16
    dpad_up = dpad in (0, 1, 7)
    dpad_down = dpad in (3, 4, 5)
    dpad_left = dpad in (5, 6, 7)
    dpad_right = dpad in (1, 2, 3)
    square, cross, circle, triangle = [get_bit(hid, 5, a) for a in [4, 5, 6, 7]]
    l1, r1, l2, r2, share, opt, l3, r3 = [get_bit(hid, 6, a) for a in range(8)]
    left = (hid[1], hid[2])
    right = (hid[3], hid[4])
    l2_analog = hid[8]
    r2_analog = hid[9]
    x360_buttons = 0
    for mask, name in mapping.items():
        try:
            name = convert[name]
        except:
            pass
        if locals()[name]:
            x360_buttons = x360_buttons | mask
    if left[0] != prev["x"] or left[1] != prev["y"] and reports % 100 == 1:
        prev["x"] = left[0]
        prev["y"] = left[1]
        plt.figure(figsize=(100,100))
        plt.subplot(211)
        plt.scatter(((left[0]-128)/127)*32767, ((left[1]-128)/127)*32767)
        plt.subplot(212)
        plt.scatter(correct(left[0], 0, 255, 5), correct(left[1], 0, 255,5))
        plt.pause(0.001)
    x360_axis = [correct(v, 0, 255, 5) for v in left+right]
    #print(x360_axis)
    return x360_axis, x360_buttons, l2_analog, r2_analog
    #print(x360_buttons)
    #if left[0]<120:
    #    x360_th_lx
    #print(dpad_up, l2_analog, square, triangle, circle, cross, opt, share, left, right)

def reader():
    global reports
    reports = reports + 1
    buf = bytearray(77)
    ret = tfd.readinto(buf)
    # change
    if ret < 77 or buf[0] != 0x11:
        print(ret)
        return
    q = memoryview(buf)[2:]
    a, b, l, r = parse(q)
    s = struct.pack('<BHBBhhhh', 1, b, l, r, *a)
    for addr, transport in clients.items():
        transport.sendto(s, addr)

def p():
    global reports
    print(reports/5)
    reports = 0
    loop.call_later(5, p)

#ply = plt()
plt.ion()
loop = asyncio.get_event_loop()
listen = loop.create_datagram_endpoint(
    UDPProto, local_addr=('192.168.1.65', 9999))
loop.add_reader(report_fd, reader)
loop.call_later(5, p)
transport, protocol = loop.run_until_complete(listen)
try:
    loop.run_forever()
finally:
    tfd.close()
