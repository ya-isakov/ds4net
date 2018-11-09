import asyncio
from io import FileIO
import os
from select import epoll, EPOLLIN
import struct
import binascii
#import numpy

prev = {"x": 0, "y": 0}

report_fd = os.open("/dev/hidraw2", os.O_RDWR | os.O_NONBLOCK)
tfd = FileIO(report_fd, "w+", closefd=False)
reports = 0
clients = {}
controls = {
    'small_rumble': 0,
    'big_rumble': 0,
    'r': 0,
    'g': 0,
    'b': 255,
    'volume_l': 80,
    'volume_r': 80
}

DEFAULT_LATENCY = 4

def connection(data, addr, transport):
    print("connected %s %s" % addr)
    clients[addr] = transport

def rumble(data, addr, transport):
    gamepad = data[1]
    small = data[2]
    big = data[3]
    print("Rumble from %s: gamepad %d, small %d, big %d" %(
          addr, gamepad, small, big))
    controls['small_rumble'] = small
    controls['big_rumble'] = big
    control()

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

def get_control(name):
    return max(min(controls.pop(name, 0), 255), 0)

def control():
    pkt = bytearray(74)
    pkt[0] = 0x11
    pkt[1] = 0xC0 | DEFAULT_LATENCY
    pkt[3] = 0x07
    pkt[6] = get_control('small_rumble')
    pkt[7] = get_control('big_rumble')
    pkt[8] = get_control('r')
    pkt[9] = get_control('g')
    pkt[10] = get_control('b')
    # Time to flash bright (255 = 2.5 seconds)
    pkt[11] = 0 # min(flash_led1, 255)
    # Time to flash dark (255 = 2.5 seconds)
    pkt[12] = 0 # min(flash_led2, 255)
    pkt[21] = get_control('volume_l')
    pkt[22] = get_control('volume_r')
    pkt[23] = 0x49 # magic
    pkt[24] = get_control('volume_speaker')
    pkt[25] = 0x85 # magic
    t = bytearray([0xA2])+pkt
    pkt = pkt+struct.pack("<L", binascii.crc32(t))
    tfd.write(pkt)

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
    x360_axis = [correct(v, 0, 255, 5) for v in left+right]
    return x360_axis, x360_buttons, l2_analog, r2_analog

def reader():
    global reports
    reports = reports + 1
    buf = bytearray(77)
    ret = tfd.readinto(buf)
    print(buf)
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
    print("Latency", 5000/reports)
    reports = 0
    loop.call_later(5, p)

def writer():
    print("qq")

control()
#control(0, 0, 0, 255, 0)
#exit(0)
loop = asyncio.get_event_loop()
listen = loop.create_datagram_endpoint(
    UDPProto, local_addr=('127.0.0.1', 9999))
loop.add_reader(report_fd, reader)
#loop.add_writer(report_fd, writer)
loop.call_later(5, p)
transport, protocol = loop.run_until_complete(listen)
try:
    loop.run_forever()
finally:
    tfd.close()
