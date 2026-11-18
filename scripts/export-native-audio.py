#!/usr/bin/env python3
"""Derive native WAV clips from the approved recording bank and cue offsets."""
import hashlib
import json
import math
from pathlib import Path
import re
import struct
import subprocess
import sys
import wave
import zlib

root = Path(__file__).resolve().parents[1]
source = root / 'assets/audio'
out = root / 'assets/native/audio'
sha = lambda data: hashlib.sha256(data).hexdigest()
bank = source / 'job-audio-bank.mp3'
cue_source = source / 'job-audio-cues.js'
match = re.search(r'export const sampleCues\s*=\s*(\{.*\})\s*;', cue_source.read_text())
if not match:
    raise SystemExit('Expected the approved JSON cue literal; source bank is never regenerated.')
cues = json.loads(match.group(1))
sources = {'bankSha256': sha(bank.read_bytes()), 'cuesSha256': sha(cue_source.read_bytes())}
if '--check' in sys.argv:
    manifest = json.loads((out / 'manifest.json').read_text())
    assert manifest['sources'] == sources, 'Native clips do not match the approved bank.'
    for filename, checksum in manifest['files'].items():
        assert sha((out / filename).read_bytes()) == checksum, f'Changed native recording: {filename}'
    print(f"Native bank verified: {len(manifest['cues'])} cues, {len(manifest['files'])} WAV files.")
    raise SystemExit
stream = json.loads(subprocess.check_output(['ffprobe', '-v', 'error', '-select_streams', 'a:0', '-show_entries', 'stream=sample_rate,channels', '-of', 'json', str(bank)]))['streams'][0]
rate, channels = int(stream['sample_rate']), int(stream['channels'])
pcm = subprocess.check_output(['ffmpeg', '-nostdin', '-v', 'error', '-i', str(bank), '-f', 's16le', '-acodec', 'pcm_s16le', '-ar', str(rate), '-ac', str(channels), 'pipe:1'])
frame_size = channels * 2
manifest = {'version': 1, 'sources': sources, 'sampleRate': rate, 'channels': channels, 'sampleFormat': 'PCM16LE', 'cues': {}, 'files': {}}
out.mkdir(parents=True, exist_ok=True)
for name, variants in cues.items():
    records = []
    for cue in variants:
        offset, duration, gain = (float(cue[k]) for k in ['offset', 'duration', 'gain'])
        assert all(math.isfinite(x) for x in [offset, duration, gain]) and offset >= 0 and duration > 0 and 0 <= gain <= 1
        start, end = round(offset * rate), round((offset + duration) * rate)
        segment = pcm[start * frame_size:end * frame_size]
        assert len(segment) == (end - start) * frame_size, f'Cue outside the decoded bank: {name}'
        filename = sha(segment)[:24] + '.wav'
        if filename not in manifest['files']:
            with wave.open(str(out / filename), 'wb') as target:
                target.setnchannels(channels)
                target.setsampwidth(2)
                target.setframerate(rate)
                target.writeframes(segment)
            manifest['files'][filename] = sha((out / filename).read_bytes())
        records.append({'file': 'native/audio/' + filename, 'gain': gain, 'loop': bool(cue.get('loop', False))})
    manifest['cues'][name] = records

# Small vector speaker symbols rasterized without GUI, downloads or new art sources.
def speaker_png(muted):
    scale, size = 3, 96
    pixels = bytearray(size * size * 4)
    polygon = [(4, 12), (10, 12), (18, 6), (18, 26), (10, 20), (4, 20)]
    strokes = []
    if muted:
        strokes = [((22, 12), (29, 20)), ((22, 20), (29, 12))]
    else:
        for radius in [7, 11]:
            points = [(16 + radius * math.cos(math.radians(a)), 16 + radius * math.sin(math.radians(a))) for a in range(-55, 56, 5)]
            strokes.extend(zip(points, points[1:]))
    def inside(x, y):
        crossings = 0
        for a, b in zip(polygon, polygon[1:] + polygon[:1]):
            if (a[1] > y) != (b[1] > y) and x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0]: crossings += 1
        if crossings % 2: return True
        for a, b in strokes:
            dx, dy = b[0] - a[0], b[1] - a[1]
            t = max(0, min(1, ((x - a[0]) * dx + (y - a[1]) * dy) / (dx * dx + dy * dy)))
            if math.hypot(x - a[0] - t * dx, y - a[1] - t * dy) <= 1.25: return True
        return False
    for y in range(size):
        for x in range(size):
            if inside((x + .5) / scale, (y + .5) / scale):
                pos = (y * size + x) * 4
                pixels[pos:pos + 4] = bytes([255, 220, 151, 255])
    def chunk(kind, value):
        return struct.pack('>I', len(value)) + kind + value + struct.pack('>I', zlib.crc32(kind + value))
    rows = b''.join(b'\x00' + pixels[y * size * 4:(y + 1) * size * 4] for y in range(size))
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', size, size, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(rows, 9)) + chunk(b'IEND', b'')
for muted, name in [(False, 'speaker.png'), (True, 'muted.png')]:
    (out / name).write_bytes(speaker_png(muted))
(out / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, sort_keys=True, indent=2) + '\n')
for obsolete in out.glob('*.wav'):
    if obsolete.name not in manifest['files']: obsolete.unlink()
print(f"Derived {len(manifest['files'])} native WAV clips from {len(cues)} approved cues; preserved {rate}Hz/{channels} channels.")
