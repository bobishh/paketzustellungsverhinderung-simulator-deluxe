#!/usr/bin/env python3
"""Cache starter geometry for the curated districts; no raster map tiles."""
import json, math, pathlib, time, urllib.request, urllib.parse
cities=[('Berlin',52.4895,13.3908),('New York',40.7265,-73.9815),('Sao Paulo',-23.5505,-46.6892),('Tokyo',35.6616,139.6682),('Cape Town',-33.9335,18.4112),('Sydney',-33.8845,151.2133)]
root=pathlib.Path(__file__).resolve().parent.parent/'cache/osm';root.mkdir(parents=True,exist_ok=True)
for name,lat,lon in cities:
    prefix=f'{lat:.5f}_{lon:.5f}'
    if all((root/f'{prefix}_{x}_{y}.json').exists() for x in range(-3,4) for y in range(-3,4)): print(name,'cached',flush=True);continue
    dy=180/6378137*180/math.pi;dx=dy/math.cos(math.radians(lat))
    box=f'{lat-3*dy},{lon-3*dx},{lat+4*dy},{lon+4*dx}'
    query=f'[out:json][timeout:35];(way["building"]({box});way["highway"]({box}););out geom;'
    result=None
    for host in ['overpass.openstreetmap.fr','overpass.kumi.systems']:
        try:
            req=urllib.request.Request(f'https://{host}/api/interpreter',urllib.parse.urlencode({'data':query}).encode(),{'User-Agent':'PaketSimulator/0.1 (public OSM neighbourhood game)'})
            with urllib.request.urlopen(req,timeout=50) as r: result=json.load(r)
            if not result.get('elements'): raise ValueError('No geometry')
            break
        except Exception as e: print(name,host,str(e),flush=True)
    if not result:continue
    for x in range(-3,4):
        for y in range(-3,4):
            elements=[]
            for e in result['elements']:
                g=e.get('geometry',[])
                if not g:continue
                if max(p['lat'] for p in g)<lat+y*dy or min(p['lat'] for p in g)>lat+(y+1)*dy or max(p['lon'] for p in g)<lon+x*dx or min(p['lon'] for p in g)>lon+(x+1)*dx:continue
                elements.append(e)
            (root/f'{prefix}_{x}_{y}.json').write_text(json.dumps({'version':0.6,'elements':elements},separators=(',',':')))
    print(name,len(result['elements']),'ways cached',flush=True)
    time.sleep(1)
