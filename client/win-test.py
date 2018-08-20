import ctypes
import socket
import struct

class Target(ctypes.Structure):
	_fields_ = [("Size", ctypes.c_ulong),
		    ("SerialNo", ctypes.c_ulong),
		    ("State", ctypes.c_uint),
		    ("VendorId", ctypes.c_ushort),
		    ("ProductId", ctypes.c_ushort)]

class Report(ctypes.Structure):
	_fields_ = [("wButtons", ctypes.c_ushort),
		    ("bLeftTrigger", ctypes.c_byte),
		    ("bRightTrigger", ctypes.c_byte),
		    ("sThumbLX", ctypes.c_short),
		    ("sThumbLY", ctypes.c_short),
		    ("sThumbRX", ctypes.c_short),
		    ("sThumbRY", ctypes.c_short)]

def run_and_check_result(func, *args, **kwargs):
	res = func(*args, **kwargs)
	if res != 0x20000000:
		raise Exception("Wrong error code: %s" % res)

def vigem_init():
	dll = ctypes.CDLL("ViGEmUM.dll")
	dll.vigem_init.restype = ctypes.c_uint
	dll.vigem_target_plugin.restype = ctypes.c_uint
	dll.vigem_target_unplug.restype = ctypes.c_uint
	dll.vigem_shutdown.restype = ctypes.c_uint
	dll.vigem_register_xusb_notification.restype = ctypes.c_uint
	#dll.vigem_unregister_xusb_notification.restype = ctypes.c_uint
	run_and_check_result(dll.vigem_init)
	return dll

def vigem_plug(dll):
	target = Target(State=1, Size=ctypes.sizeof(Target))
	p_target = ctypes.byref(target)
	run_and_check_result(dll.vigem_target_plugin, 0, p_target)
	return target

def vigem_register_callback(dll, target, callback):
	#c_callback = CALLBACK(callback)	
	run_and_check_result(dll.vigem_register_xusb_notification, callback, target)

@ctypes.CFUNCTYPE(None, Target, ctypes.c_ubyte, ctypes.c_ubyte, ctypes.c_ubyte)
def rumble_callback(Target, Large, Small, LedNum):
	print(Target, Large, Small, LedNum)
	sock.send(bytearray([2,1, Large, Small]))

def udp_init(ip, port):
	sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
	sock.connect((ip, port))
	sock.send(bytearray([0]))
	return sock

dll = vigem_init()
sock = udp_init("192.168.1.65", 9999)
target = vigem_plug(dll)
vigem_register_callback(dll, target, rumble_callback)

try:
	while True:
		ret = sock.recv(13)
		ret = struct.unpack("<BHBBhhhh", ret)
		#print(ret[1:4])
		[check(ret[x]) for x in [4, 5, 6, 7]]
		r = Report(wButtons=ret[1], bLeftTrigger=ret[2], bRightTrigger=ret[3], sThumbLX=ret[4], sThumbLY=-ret[5], sThumbRX=ret[6], sThumbRY=-ret[7])
		#r=Report()
		dll.vigem_xusb_submit_report(target, r)
		#sleep(1)
finally:
	run_and_check_result(dll.vigem_target_unplug, target)
	sock.send(bytearray([1]))
	res = dll.vigem_shutdown()