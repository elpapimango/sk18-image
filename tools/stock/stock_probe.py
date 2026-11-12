#!/usr/bin/env python3
"""Talks to an SK-18 over its serial deck link and records exactly what it answers.

Run it on a Linux host (the DietPi) with the *stock* SD card in the SK-18, then again with the open image, and compare
the two logs (compare mode below). Only methods that do not change the device's storage are sent: no saveToFile,
setFileCRC, deleteFiles, keyboardInput or playAudio.

    python3 stock_probe.py probe  /dev/ttyACM0 stock.jsonl [--touch-seconds 20]
    python3 stock_probe.py probe  tcp:192.168.0.50:7000 esp.jsonl      (a deck on the network, from any OS)
    python3 stock_probe.py compare stock.jsonl open.jsonl

Stop whatever else uses the port first (on the DietPi: `sudo systemctl stop control-deck-gatewayd`).
The log has one JSON object per line: what was sent (`tx`), what came back (`rx`, the decoded frame), the time since
the request in milliseconds, and `raw` (hex of the bytes) for anything that is not a well-formed frame.
Standard library only.
"""
import argparse
import binascii
import glob
import json
import os
import select
import socket
import struct
import sys
import time
import zlib

HEADER = b"\xA1\xA5\x5A\x5E"
CMD_JPG, CMD_JSON, CMD_END = 100, 101, 102


def crc(data):
    return zlib.crc32(data) & 0xFFFFFFFF


def frame(fid, cmd, data):
    size = struct.pack("<I", len(data))
    return HEADER + struct.pack("<II", fid, cmd) + size + struct.pack("<I", crc(size)) + data + struct.pack("<I", crc(data))


def request(fid, method, params=None):
    body = {"method": method}
    if params is not None:
        body["parameters"] = params
    return frame(fid, CMD_JSON, json.dumps(body, separators=(",", ":")).encode())


class Parser:
    """Cuts frames out of a byte stream the way the stock firmware does: resync on the header, drop bad CRCs."""

    def __init__(self):
        self.buf = b""
        self.dropped = 0

    def feed(self, data):
        self.buf += data
        out = []
        while True:
            at = self.buf.find(HEADER)
            if at < 0:
                self.buf = self.buf[-3:]
                break
            self.buf = self.buf[at:]
            if len(self.buf) < 20:
                break
            fid, cmd, size, size_crc = struct.unpack("<IIII", self.buf[4:20])
            if crc(self.buf[12:16]) != size_crc:
                self.buf = self.buf[1:]
                self.dropped += 1
                continue
            if len(self.buf) < 20 + size + 4:
                break
            payload = self.buf[20 : 20 + size]
            (data_crc,) = struct.unpack("<I", self.buf[20 + size : 24 + size])
            self.buf = self.buf[24 + size :]
            if crc(payload) != data_crc:
                self.dropped += 1
                continue
            out.append({"id": fid, "cmd": cmd, "size": size, "data": payload})
        return out


def open_port(path):
    if path.startswith("tcp:"):
        host, _, port = path[4:].rpartition(":")
        sock = socket.create_connection((host, int(port)), timeout=5)
        sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        return sock
    import termios  # Unix only; the framing code above works anywhere
    import tty

    fd = os.open(path, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
    tty.setraw(fd)
    attrs = termios.tcgetattr(fd)
    attrs[4] = attrs[5] = termios.B115200
    termios.tcsetattr(fd, termios.TCSANOW, attrs)
    return fd


class Link:
    """The open port; when the device resets itself (it re-enumerates, as /dev/ttyACM<n> with a new n) the probe
    waits for it to come back, notes that in the log and goes on."""

    def __init__(self, port, write):
        self.port, self.write_log, self.parser = port, write, Parser()
        self.fd = open_port(port)

    def send(self, data, label):
        try:
            if isinstance(self.fd, socket.socket):
                self.fd.sendall(data)
            else:
                os.write(self.fd, data)
            return True
        except OSError as error:
            self.write_log({"event": "device went away while sending", "before": label, "error": str(error)})
            self.reopen()
            return False

    def listen(self, seconds):
        try:
            return listen(self.fd, self.parser, seconds)
        except OSError as error:
            self.write_log({"event": "device went away while listening", "error": str(error)})
            self.reopen()
            return []

    def reopen(self):
        try:
            self.fd.close() if isinstance(self.fd, socket.socket) else os.close(self.fd)
        except OSError:
            pass
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            ports = [self.port] if self.port.startswith("tcp:") else sorted(glob.glob("/dev/ttyACM*"))
            if ports:
                time.sleep(1.0)
                self.port = ports[-1]
                self.fd = open_port(self.port)
                self.parser = Parser()
                self.write_log({"event": "device is back", "port": self.port})
                return
            time.sleep(0.25)
        raise SystemExit("the device did not come back")


def listen(fd, parser, seconds):
    """Frames that arrive within `seconds`, with the time each came at."""
    frames, start = [], time.monotonic()
    while time.monotonic() - start < seconds:
        ready, _, _ = select.select([fd], [], [], 0.05)
        if ready:
            try:
                chunk = fd.recv(65536) if isinstance(fd, socket.socket) else os.read(fd, 65536)
            except (BlockingIOError, socket.timeout):
                continue
            if isinstance(fd, socket.socket) and not chunk:
                raise ConnectionResetError("the deck closed the connection")
            for f in parser.feed(chunk):
                f["ms"] = round((time.monotonic() - start) * 1000, 1)
                frames.append(f)
    return frames


def describe(f):
    out = {"id": f["id"], "cmd": f["cmd"], "size": f["size"], "ms": f.get("ms")}
    if f["cmd"] == CMD_JSON:
        try:
            out["json"] = json.loads(f["data"])
        except ValueError:
            out["raw"] = binascii.hexlify(f["data"]).decode()
    else:
        out["raw"] = binascii.hexlify(f["data"][:64]).decode()
    return out


# What is sent: (label, method, parameters). Nothing here writes the device's storage.
PROBES = [
    ("info", "getInfo", None),
    ("info again", "getInfo", None),
    ("backlight 70", "setBacklight", {"level": 70}),
    ("backlight 100", "setBacklight", {"level": 100}),
    ("backlight 0", "setBacklight", {"level": 0}),
    ("backlight 100 again", "setBacklight", {"level": 100}),
    ("backlight no level", "setBacklight", {}),
    ("backlight 300", "setBacklight", {"level": 300}),
    ("backlight text", "setBacklight", {"level": "high"}),
    ("volume 50", "setVolume", {"level": 50}),
    ("stop audio", "stopAudio", None),
    ("files by suffix", "getFilesBySuffix", {"suffixs": ["jpg", "mp3"]}),
    ("unknown method", "noSuchMethod", None),
    ("getSettings (open firmware only)", "getSettings", None),
    ("method without parameters key", "setVolume", None),
    ("volume 3", "setVolume", {"level": 3}),
    ("volume 0", "setVolume", {"level": 0}),
    ("volume negative", "setVolume", {"level": -1}),
    ("backlight negative", "setBacklight", {"level": -1}),
    ("backlight fraction", "setBacklight", {"level": 55.5}),
    ("backlight null", "setBacklight", {"level": None}),
    ("backlight bool", "setBacklight", {"level": True}),
    ("backlight parameters not an object", "setBacklight", [1]),
    ("play audio missing file", "playAudio", {"filePath": "/nonexistent.mp3"}),
    ("play audio no path", "playAudio", None),
    ("files by suffix no parameters", "getFilesBySuffix", None),
    ("files by suffix empty list", "getFilesBySuffix", {"suffixs": []}),
    ("keyboard input no parameters", "keyboardInput", None),
    ("backlight restored to 100", "setBacklight", {"level": 100}),
]


def probe(port, out_path, touch_seconds, touch_only=False):
    log = open(out_path, "w", encoding="utf-8")

    def write(entry):
        entry["t"] = round(time.time(), 3)
        log.write(json.dumps(entry, sort_keys=True) + "\n")
        log.flush()
        print(json.dumps(entry, sort_keys=True)[:200], flush=True)

    link = Link(port, write)
    if touch_only:
        print(f"Touch some keys now ({touch_seconds} s)...", flush=True)
        write({"label": "touch events", "rx": [describe(f) for f in link.listen(touch_seconds)]})
        return
    link.listen(0.3)  # anything the device says unasked
    write({"note": "unsolicited bytes before the first request", "raw": binascii.hexlify(link.parser.buf).decode()})
    fid = 1000

    def ask(label, data, wait, **extra):
        sent = time.monotonic()
        ok = link.send(data, label)
        replies = link.listen(wait) if ok else []
        entry = {"label": label, "rx": [describe(f) for f in replies], "wall_s": round(time.monotonic() - sent, 3)}
        entry["ms_to_first"] = replies[0]["ms"] if replies else None
        entry.update(extra)
        write(entry)

    for label, method, params in PROBES:
        fid += 1
        ask(label, request(fid, method, params), 1.5, tx={"id": fid, "method": method, "parameters": params})
        time.sleep(0.3)
    # A frame with a bad data CRC, then a good one: does the device recover, and does it answer the bad one?
    fid += 1
    bad = bytearray(request(fid, "getInfo"))
    bad[-1] ^= 0xFF
    ask("bad data crc", bytes(bad), 1.0)
    fid += 1
    ask("getInfo after the bad frame", request(fid, "getInfo"), 1.5)
    # Garbage in front of a good frame (the vendor demo sends a megabyte of '0' first).
    fid += 1
    ask("4 KiB of '0' before getInfo", b"0" * 4096 + request(fid, "getInfo"), 2.0)
    # A tiny picture frame: how is a JPEG answered? (an invalid JPEG; the answer shows whether the echo is sent regardless)
    fid += 1
    ask("empty JPEG (SOI+EOI)", frame(fid, CMD_JPG, bytes([0xFF, 0xD8, 0xFF, 0xD9])), 2.0)
    if touch_seconds:
        print(f"Touch some keys now ({touch_seconds} s)...", flush=True)
        replies = link.listen(touch_seconds)
        write({"label": "touch events", "rx": [describe(f) for f in replies]})
    link.fd.close() if isinstance(link.fd, socket.socket) else os.close(link.fd)
    log.close()


def shape(value):
    """The structure of a JSON value without the values that vary (numbers, strings)."""
    if isinstance(value, dict):
        return {k: shape(v) for k, v in sorted(value.items())}
    if isinstance(value, list):
        return [shape(value[0])] if value else []
    return type(value).__name__


def compare(a_path, b_path):
    load = lambda p: [json.loads(l) for l in open(p, encoding="utf-8")]
    a, b = load(a_path), load(b_path)
    same = True
    for x, y in zip(a, b):
        label = x.get("label", x.get("note"))
        rx_a, rx_b = x.get("rx", []), y.get("rx", [])
        sig = lambda rx: [(r["cmd"], shape(r.get("json"))) for r in rx]
        if sig(rx_a) != sig(rx_b):
            same = False
            print(f"DIFFERENT  {label}")
            print("  A:", json.dumps([r.get("json", r.get("raw")) for r in rx_a])[:300])
            print("  B:", json.dumps([r.get("json", r.get("raw")) for r in rx_b])[:300])
        else:
            print(f"same shape {label}")
    sys.exit(0 if same else 1)


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="mode", required=True)
    p = sub.add_parser("probe")
    p.add_argument("port")
    p.add_argument("log")
    p.add_argument("--touch-seconds", type=int, default=0)
    p.add_argument("--touch-only", action="store_true", help="skip the requests, only record key events")
    c = sub.add_parser("compare")
    c.add_argument("a")
    c.add_argument("b")
    args = ap.parse_args()
    if args.mode == "probe":
        probe(args.port, args.log, args.touch_seconds, args.touch_only)
    else:
        compare(args.a, args.b)
