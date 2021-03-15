package main

import (
	"encoding/binary"
	"fmt"
	"hash/crc32"
	"io"
	"log"
	"net"
	"os"
	"sync"
	"time"
)

type Color struct {
	red   uint8
	green uint8
	blue  uint8
}

type Haptic struct {
	big   uint8
	small uint8
}

var events int = 0
var lost int = 0
var clients = make(map[*net.UDPAddr]bool, 0)
var latency uint8 = 20
var color = Color{red: 0, green: 0, blue: 0xFF}
var frame_number uint16 = 0
var sound_buffer = make([]byte, 448)
var sound_lock sync.Mutex
var sound_new_data bool = false

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
	if buttons > 0 {
		fmt.Println(buttons)
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
	buffer := make([]byte, 78)
	var last uint8 = 0
	var counter uint8 = 0
	var diff uint8
	for {
		count, err := f.Read(buffer)
		if err != nil {
			log.Fatal(err)
		}
		if count != 78 && buffer[0] != 0x11 {
			fmt.Printf("Bad buffer %v\n", buffer)
		}
		counter = buffer[9] >> 2
		if counter > last {
			diff = counter - last - 1
		} else if counter < last {
			diff = counter + 64 - last - 1
		} else {
			fmt.Println("Same counter!")
			diff = 0
		}
		if diff > 0 {
			lost = lost + int(diff)
			fmt.Println(lost)
		}
		last = counter
		packet := convert_hid_to_packet(buffer[2:])
		events = events + 1
		for client := range clients {
			conn.WriteTo(packet, client)
		}
	}
}

func control(f *os.File, latency uint8, color Color, haptic Haptic) {
	buffer := make([]byte, 74)
	buffer[0] = 0x11
	buffer[1] = 0xC0 | latency
	buffer[2] = 0x20
	buffer[3] = 0xFF
	buffer[6] = haptic.small
	buffer[7] = haptic.big
	buffer[8] = color.red
	buffer[9] = color.green
	buffer[10] = color.blue
	buffer[21] = 40
	buffer[22] = 40
	crc := crc32.Checksum(append([]byte{0xA2}, buffer...), crc32.IEEETable)
	checksum := make([]byte, 4)
	binary.LittleEndian.PutUint32(checksum, crc)
	buf := append(buffer, checksum...)
	n, err := f.Write(buf)
	if err != nil {
		log.Println(n, err)
	}
	fmt.Println(buf)
}

func frame_number_increase(inc uint16) []byte {
	buf := make([]byte, 2)
	binary.LittleEndian.PutUint16(buf, frame_number)
	frame_number += inc
	return buf
}

func send_sound(f *os.File, audio []byte) {
	//for {
	//	if sound_new_data {
			frame_no := frame_number_increase(4)
			header := make([]byte, 6)
			header[0] = 0x17
			header[1] = 0x40
			header[2] = 0xA0
			header[3] = frame_no[0]
			header[4] = frame_no[1]
			header[5] = 0x24
			no := make([]byte, 4)
			//sound_lock.Lock()
			audio_temp := append(audio, no...)
			sound_new_data = false
			//sound_lock.Unlock()
			data := append(header, audio_temp...)
			crc := crc32.Checksum(append([]byte{0xA2}, data...), crc32.IEEETable)
			checksum := make([]byte, 4)
			binary.LittleEndian.PutUint32(checksum, crc)
			buf := append(data, checksum...)
			_, err := f.Write(buf)
			if err != nil {
				//log.Println(n, err)
			}
			//fmt.Println(buf, len(buf))
	//	} else {
	//	    time.Sleep(time.Millisecond)
	//	}
	//}
}

func sound_reader(f *os.File) {
	buf := make([]byte, 448)
	for {
		if _, err := io.ReadFull(os.Stdin, buf); err != nil {
			log.Println(err)
			return
		}
		if ! sound_new_data {
		    //sound_lock.Lock()
		    sound_buffer = buf
		    sound_new_data = true
		    //sound_lock.Unlock() 
		} else {
		    sound_buffer = buf
		}
		send_sound(f, buf)
		//fmt.Println(buf, len(buf))
	}
}

func main() {
	buf := make([]byte, 100)
	file, err := os.OpenFile("/dev/hidraw2", os.O_RDWR, 0644)
	fmt.Println(file.pd.pollable())
	if err != nil {
		log.Fatal(err)
	}
	haptic_start := Haptic{big: 0, small: 0xFF}
	haptic_stop := Haptic{big: 0, small: 0}
	control(file, latency, color, haptic_start)
	timer := time.NewTimer(time.Second)
	go func() {
		<-timer.C
		control(file, latency, color, haptic_stop)
	}()
	ticker := time.NewTicker(time.Second * 10)
	go func() {
		for range ticker.C {
			fmt.Printf("Real latency: %v Packet loss %v\n", 10000/float32(events), float32(100*lost)/float32(events+lost))
			events = 0
			lost = 0
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
	go sound_reader(file)
	//go send_sound(file)
	//go outputter()
	for {
		n, addr, err := conn.ReadFromUDP(buf)
		if err != nil {
			log.Fatal(err)
		}
		if buf[0] == 0 && n == 1 {
			fmt.Printf("Gotcha %v\n", addr)
			clients[addr] = true
		} else if buf[1] == 1 {
			//
		}
	}
	fmt.Scanln()
}
