#!/usr/bin/env python3
"""Cache Mapzen elevation tiles around the offered starter districts."""
import concurrent.futures
import math
from pathlib import Path
import urllib.request

CITIES = [(52.4895, 13.3908), (40.7265, -73.9815), (-23.5505, -46.6892),
          (35.6616, 139.6682), (-33.9335, 18.4112), (-33.8845, 151.2133)]
ROOT = Path(__file__).resolve().parent.parent / 'cache/elevation'
ZOOM = 13
RADIUS = 1600


def pixel(lat, lon):
    n = (1 << ZOOM) * 256
    return ((lon + 180) / 360 * n - .5,
            (1 - math.asinh(math.tan(math.radians(lat))) / math.pi) * n / 2 - .5)


def tiles(lat, lon):
    dy = math.degrees(RADIUS / 6378137)
    dx = dy / math.cos(math.radians(lat))
    west, north = pixel(lat + dy, lon - dx)
    east, south = pixel(lat - dy, lon + dx)
    return {(x % (1 << ZOOM), y)
            for x in range(math.floor(west / 256), math.floor((east + 1) / 256) + 1)
            for y in range(math.floor(north / 256), math.floor((south + 1) / 256) + 1)}


def download(tile):
    x, y = tile
    path = ROOT / f'{x}-{y}.png'
    if path.exists():
        return
    url = f'https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{ZOOM}/{x}/{y}.png'
    try:
        with urllib.request.urlopen(url, timeout=10) as response:
            data = response.read(512_000)
        if data[:8] != b'\x89PNG\r\n\x1a\n':
            raise ValueError('expected a PNG')
        path.write_bytes(data)
    except (OSError, ValueError) as error:
        print(f'Elevation {x}/{y}: {error}', flush=True)


if __name__ == '__main__':
    ROOT.mkdir(parents=True, exist_ok=True)
    wanted = set().union(*(tiles(*place) for place in CITIES))
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        list(pool.map(download, sorted(wanted)))
    print(f'{sum((ROOT / f"{x}-{y}.png").exists() for x, y in wanted)}/{len(wanted)} starter elevation tiles cached')
