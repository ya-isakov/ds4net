package main

import (
	"fmt"
	"log"
	"net"
	"os"
	"time"
)

type Packet struct {
	op      uint8
	buttons uint16
	left    uint8
	right   uint8
	left1   int16
	left2   int16
	right1  int16
	right2  int16
}

var events int = 0
var clients map[*net.UDPAddr]bool = make(map[*net.UDPAddr]bool, 0)

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

func to_big(data uint16) []byte {
	return []byte{byte(data % 256), byte(data >> 8)}
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

func parse_and_map(hid []byte) []byte {
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
	ret = append(ret, to_big(buttons)...)
	ret = append(ret, l2_analog)
	ret = append(ret, r2_analog)
	for _, axis := range hid[1:5] {
		ret = append(ret, to_big(uint16(correct_axis(int32(axis), 0, 255, 5)))...)
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
		packet := parse_and_map(buffer[2:])
		events = events + 1
		for client := range clients {
			conn.WriteTo(packet, client)
		}
	}
}

func outputter() {
	for {
		//<- ch
		//fmt.Printf("Read %d bytes\n", <-ch)
	}
}

func main() {
	z := []byte{5, 2}
	fmt.Printf("%v %v\n", get_bit(z, 0, 0), get_bit(z, 0, 2))
	buf := make([]byte, 100)
	file, err := os.Open("/dev/hidraw2") // For read access.
	if err != nil {
		log.Fatal(err)
	}
	ticker := time.NewTicker(time.Second * 5)
	go func() {
		for range ticker.C {
			fmt.Printf("Events per second %d\n", events/5)
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
