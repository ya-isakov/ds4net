#!/usr/bin/env python3
import binascii
import struct
from sys import stdin
import os
from io import FileIO
import signal

hiddev = os.open("/dev/hidraw2", os.O_RDWR | os.O_NONBLOCK)
pf = FileIO(hiddev, "wb+", closefd=False)
#pf=open("ds_my.bin", "wb+")

rumble_weak = 0
rumble_strong = 0
r = 0
g = 10
b = 0
crc = b'\x00\x00\x00\x00'
volume_speaker = 60
volume_l = 40
volume_r = 40
unk2 = 0x49
unk3 = 0x85
flash_bright = 0
flash_dark = 0
header = b'\x24'

class SBCFrameHeaderParser(object):

	MONO = 0
	DUAL_CHANNEL = 1
	STEREO = 2
	JOINT_STEREO = 3


	def __init__(self):
		pass

	def parse(self, raw_header):
		# Info in SBC headers from
		# https://tools.ietf.org/html/draft-ietf-avt-rtp-sbc-01#section-6.3

		# Syncword should be 0x9C
		self.syncword = raw_header[0]

		self.nrof_subbands = \
			SBCFrameHeaderParser.parse_number_of_subbands(
				raw_header
			)
		self.channel_mode = SBCFrameHeaderParser.parse_channel_mode(
			raw_header
		)
		self.nrof_channels = 2
		if self.channel_mode == SBCFrameHeaderParser.MONO:
			self.nrof_channels = 1
		self.nrof_blocks = SBCFrameHeaderParser.parse_number_of_blocks(
			raw_header
		)
		self.join = 0
		if self.channel_mode == SBCFrameHeaderParser.JOINT_STEREO:
			self.join = 1
		self.nrof_subbands = \
			SBCFrameHeaderParser.parse_number_of_subbands(
				raw_header
			)
		self.bitpool = SBCFrameHeaderParser.parse_bitpool(raw_header)
		self.sampling_frequency = SBCFrameHeaderParser.parse_sampling(
			raw_header
		)


		# Calculate frame length
		def ceildiv(a, b):
			return -(-a // b)

		if (
			(self.channel_mode == SBCFrameHeaderParser.MONO)
			or (self.channel_mode == SBCFrameHeaderParser.DUAL_CHANNEL)
		):

			self.frame_length = (
				4 + (
					4
						* self.nrof_subbands
						* self.nrof_channels
				)//8
				+ ceildiv(
					self.nrof_blocks
						* self.nrof_channels
						* self.bitpool,
					8
				)
			)
		else:
			self.frame_length = (
				4 + (
					4
						* self.nrof_subbands
						* self.nrof_channels
				)//8
				+ ceildiv(
					self.join
						* self.nrof_subbands
					+ self.nrof_blocks
						* self.bitpool,
					8
				)
			)


		# Calculate bit rate
		self.bit_rate = (
			8 * self.frame_length * self.sampling_frequency
				// self.nrof_subbands // self.nrof_blocks
		)

	def print_values(self):
		# Info in SBC headers from
		# https://tools.ietf.org/html/draft-ietf-avt-rtp-sbc-01#section-6.3

		print("syncword: ", self.syncword)

		print("nrof_subbands", self.nrof_subbands)
		print("channel_mode", [
			"MONO", "DUAL_CHANNEL", "STEREO", "JOINT_STEREO"
			][self.channel_mode]
		)
		print("nrof_channels", self.nrof_channels)
		print("nrof_blocks", self.nrof_blocks)
		print("join: ", self.join)
		print("nrof_subbands", self.nrof_subbands)
		print("bitpool", self.bitpool)
		print("sampling_frequency", self.sampling_frequency)
		print("frame_length", self.frame_length)
		print("bit_rate", self.bit_rate)


	@staticmethod
	def parse_sampling(raw_header):

		sf_word = raw_header[1]

		# Find sampling frequency from rightmost 2 bits
		if sf_word & 0x80 == 0x80:
			bit_0 = 1
		else:
			bit_0 = 0

		if sf_word & 0x40 == 0x40:
			bit_1 = 1
		else:
			bit_1 = 0

		if (bit_0 == 0) and (bit_1 == 0):
			sampling_frequency = 16000
		elif (bit_0 == 0) and (bit_1 == 1):
			sampling_frequency = 32000
		elif (bit_0 == 1) and (bit_1 == 0):
			sampling_frequency = 44100
		elif (bit_0 == 1) and (bit_1 == 1):
			sampling_frequency = 48000

		return sampling_frequency

	@staticmethod
	def parse_number_of_blocks(raw_header):

		nb_word = raw_header[1]

		if nb_word & 0x20 == 0x20:
			bit_0 = 1
		else:
			bit_0 = 0

		if nb_word & 0x10 == 0x10:
			bit_1 = 1
		else:
			bit_1 = 0


		if (bit_0 == 0) and (bit_1 == 0):
			number_of_blocks = 4
		elif (bit_0 == 0) and (bit_1 == 1):
			number_of_blocks = 8
		elif (bit_0 == 1) and (bit_1 == 0):
			number_of_blocks = 12
		elif (bit_0 == 1) and (bit_1 == 1):
			number_of_blocks = 16

		return number_of_blocks

	@staticmethod
	def parse_channel_mode(raw_header):

		ch_word = raw_header[1]

		if ch_word & 0x08 == 0x08:
			bit_0 = 1
		else:
			bit_0 = 0

		if ch_word & 0x04 == 0x04:
			bit_1 = 1
		else:
			bit_1 = 0

		if (bit_0 == 0) and (bit_1 == 0):
			channel_mode = SBCFrameHeaderParser.MONO
		elif (bit_0 == 0) and (bit_1 == 1):
			channel_mode = SBCFrameHeaderParser.DUAL_CHANNEL
		elif (bit_0 == 1) and (bit_1 == 0):
			channel_mode = SBCFrameHeaderParser.STEREO
		elif (bit_0 == 1) and (bit_1 == 1):
			channel_mode = SBCFrameHeaderParser.JOINT_STEREO

		return channel_mode


	@staticmethod
	def parse_number_of_subbands(raw_header):
		if raw_header[1] & 0x01 == 0x01:
			number_of_subbands = 8
		else:
			number_of_subbands = 4

		return number_of_subbands


	@staticmethod
	def parse_bitpool(raw_header):
		return int(raw_header[2])


def frame_number(inc):
	res = struct.pack("<H", frame_number.n)
	frame_number.n += inc
	if frame_number.n > 0xffff:
		frame_number.n = 0
	return res
frame_number.n = 0

def joy_data():
	data = [0xff,0x4,0x00]
	global volume_unk1,volume_unk2, unk3
	data.extend([rumble_weak,rumble_strong,r,g,b,flash_bright,flash_dark])
	data.extend([0]*8)
	data.extend([volume_l,volume_r,unk2,volume_speaker,unk3])
	return data

def _11_report():
	data = joy_data()
	data.extend([0]*(48))
	pkt = b'\x11\xC0\x20' + bytearray(data)
	t = bytearray([0xA2])+pkt
	pkt = pkt+struct.pack("<L", binascii.crc32(t))
	return pkt

def _14_report(audo_data):
	pkt = b'\x14\x40\xA0'+ frame_number(2) + header + audo_data + bytearray(36)
	t = bytearray([0xA2])+pkt
	pkt = pkt+struct.pack("<L", binascii.crc32(t))
	return pkt

def _15_report(audo_data):
	data = joy_data()
	data.extend([0]*(52))
	pkt = b'\x15\xC0\xA0' + bytearray(data)+ frame_number(2) + header + audo_data + bytearray(25)
	t = bytearray([0xA2])+pkt
	pkt = pkt+struct.pack("<L", binascii.crc32(t))
	return pkt

def _17_report(audo_data):
	pkt = b'\x17\x40\xA0' + frame_number(4) + header + audo_data + bytearray(4)
	t = bytearray([0xA2])+pkt
	pkt = pkt+struct.pack("<L", binascii.crc32(t))
	#[print(z, end=" ") for z in pkt]
	#print(len(pkt))
	return pkt

stdin = stdin.detach()
data = bytearray()
count = 1
pf.write(_11_report())

def sigalrm_handler(signum, frame):
    raise TimeoutError

signal.signal(
	    signal.SIGALRM, sigalrm_handler
)
while True:
	#if count % 6:
	#if True:
	#	data = _14_report(stdin.read(224)) if count % 3 else _15_report(stdin.read(224))
	#else
	data = stdin.read(224)
	#z = SBCFrameHeaderParser()
	#z.parse(data)
	#z.print_values()
	data = _14_report(data)
	#count+=1
	signal.setitimer(signal.ITIMER_REAL, 224/224000/2)
	try:
	    pf.write(data)
	except TimeoutError:
	    print("Timeout")
	finally:
	    signal.setitimer(signal.ITIMER_REAL, 0)
