package main

import (
	"encoding/binary"
	"fmt"
	"hash/crc32"
	"log"
	"net"
	"os"
	"time"
)

var events int = 0
var clients = make(map[*net.UDPAddr]bool, 0)

var dpads_map = map[byte]uint16{0: 0x1, 1: 0x9, 2: 0x8, 3: 0xA, 4: 0x2, 5: 0x6, 6: 0x4, 7: 0x5}
var buttons_map = map[int]uint16{4: 0x4000, 5: 0x1000, 6: 0x2000, 7: 0x8000}
var misc_map = map[int]uint16{0: 0x100, 1: 0x200, 4: 0x20, 5: 0x10, 6: 0x40, 7: 0x80}

func get_bit(data []byte, num int, bit uint) bool {
	return (data[num] & (1 << bit)) != 0
}

func set_if_true(value uint16, mask uint16, check bool) uint16 {
	if check {
		return value | mask
	} else {
		return value
	}
}

func clamp(v int32) int32 {
	min := int32(32767)
	if v < 32767 {
		min = v
	}
	max := min
	if max < -32767 {
		max = -32767
	}
	return max
}

func correct_axis(v, min, max, dz int32) int32 {
	t1 := (max + min) / 2
	c0 := t1 - dz
	c1 := t1 + dz
	t2 := (max - min - 4*dz) / 2
	c2 := (1 << 29) / t2
	r0 := (c2 * (v - c0)) >> 14
	r1 := (c2 * (v - c1)) >> 14
	if v < c0 {
		return clamp(r0)
	} else if v > c1 {
		return clamp(r1)
	}
	return 0
}

func convert_hid_to_packet(hid []byte) []byte {
	buttons := dpads_map[hid[5]%16]
	for k, v := range buttons_map {
		buttons = set_if_true(buttons, v, get_bit(hid, 5, uint(k)))
	}
	for k, v := range misc_map {
		buttons = set_if_true(buttons, v, get_bit(hid, 6, uint(k)))
	}
	l2_analog := byte(hid[8])
	r2_analog := byte(hid[9])

	ret := []byte{1}
	bs := make([]byte, 2)
	binary.LittleEndian.PutUint16(bs, buttons)
	ret = append(ret, bs...)
	ret = append(ret, l2_analog)
	ret = append(ret, r2_analog)
	for _, axis := range hid[1:5] {
		binary.LittleEndian.PutUint16(bs, uint16(correct_axis(int32(axis), 0, 255, 5)))
		ret = append(ret, bs...)
	}
	return ret
}

func reader(f *os.File, conn *net.UDPConn) {
	buffer := make([]byte, 100)
	for {
		count, err := f.Read(buffer)
		if err != nil {
			log.Fatal(err)
		}
		if count != 78 && buffer[0] != 0x11 {
			fmt.Printf("Bad buffer %v\n", buffer)
		}
		packet := convert_hid_to_packet(buffer[2:])
		events = events + 1
		for client := range clients {
			conn.WriteTo(packet, client)
		}
	}
}

func control(f *os.File) {
	buffer := make([]byte, 74)
	buffer[0] = 0x11
	buffer[1] = 0xC0 | 4
	buffer[3] = 0x07
	buffer[9] = 0xFF
	crc := crc32.Checksum(append([]byte{0xA2}, buffer...), crc32.IEEETable)
	checksum := make([]byte, 4)
	binary.LittleEndian.PutUint32(checksum, crc)
	buf := append(buffer, checksum...)
	f.Write(buf)
	fmt.Println(buf)
}

func main() {
	buf := make([]byte, 100)
	file, err := os.OpenFile("/dev/hidraw2", os.O_RDWR, 0644)
	if err != nil {
		log.Fatal(err)
	}
	control(file)
	ticker := time.NewTicker(time.Second * 10)
	go func() {
		for range ticker.C {
			fmt.Printf("Events per second %v\n", events/10)
			events = 0
		}
	}()
	udpAddr, err := net.ResolveUDPAddr("udp4", ":9999")
	if err != nil {
		log.Fatal(err)
	}
	conn, err := net.ListenUDP("udp", udpAddr)
	if err != nil {
		log.Fatal(err)
	}
	go reader(file, conn)
	//go outputter()
	for {
		n, addr, err := conn.ReadFromUDP(buf)
		if err != nil {
			log.Fatal(err)
		}
		if buf[0] == 0 && n == 1 {
			fmt.Printf("Gotcha %v\n", addr)
			clients[addr] = true
		}
	}
	fmt.Scanln()
}
