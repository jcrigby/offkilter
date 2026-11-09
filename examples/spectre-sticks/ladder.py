#!/usr/bin/env python3
"""A trial of the cassette as a ladder: stick 1's first saw cassette as
three 22 mm stands at the ends and the middle of a 200 mm blank, joined
by two 8 mm columns along the back, modelled with the feed along Z so
the STL is in print orientation (the nest profile built up in layers,
no overhangs). Run after build.py; writes out/ladder_stick_1_saw_1.stl
and two pictures. A third of a full-length cassette's plastic and time.
"""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from okmcp import Mcp, Part
n = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out', 'sticks.json')))
fx = n['fixtures'][0][0]                     # stick 1, saw cut 1
B, Z = n['saw_footprint']['B'], n['saw_footprint']['Z']
L, STAND, COL = 200.0, 22.0, 8.0
starts = [0.0, (L - STAND) / 2, L - STAND]
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out', 'ladder.okpart')
if os.path.exists(OUT): os.remove(OUT)
mcp = Mcp(OUT); mcp.call('create_document', {'name': 'ladder'})
p = Part(mcp, 1, 'Ladder')
def slab(name, pts, z0, z1):
    s = p.sketch('top', z0, name); p.polygon(s, pts); f = p.extrude(s, z1 - z0, name=name)
    p.apply({'type': 'rename_part', 'source': f, 'name': name}); return f
parts = []
for i, z0 in enumerate(starts):
    base = slab(f'Stand {i+1}', fx['base'], z0, z0 + STAND)
    nest = slab(f'Nest {i+1}', fx['nest'], z0 - 1.0, z0 + STAND + 1.0)
    p.feature({'type': 'add_boolean', 'op': 'subtract', 'targets': [base], 'tools': [nest], 'name': f'Stand {i+1}'})
    parts.append(base)
for j, (y0, y1) in enumerate([(0.0, COL), (Z - COL, Z)]):
    parts.append(slab(f'Column {j+1}', [(-B, y0), (-B + COL, y0), (-B + COL, y1), (-B, y1)], 0.0, L))
p.feature({'type': 'add_boolean', 'op': 'union', 'targets': [parts[0]], 'tools': parts[1:], 'name': 'Ladder'})
r = p.report()
bad = [(f['name'], f['error']) for f in r['features'] if f.get('error')]
if bad: raise SystemExit(bad)
bodies = r['bodies']; print([(b['name'], round(b['volume'] / 1000, 1)) for b in bodies])
body = bodies[0]['name']
dst = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out', 'ladder_stick_1_saw_1.stl')
print(mcp.call('export', {'tab': 1, 'format': 'stl', 'body': body, 'path': dst}))
mcp.call('screenshot', {'tab': 1, 'view': 'iso', 'width': 1000, 'height': 900, 'path': os.path.join(os.path.dirname(dst), 'ladder_iso.png')})
mcp.call('screenshot', {'tab': 1, 'view': 'top', 'width': 800, 'height': 800, 'path': os.path.join(os.path.dirname(dst), 'ladder_top.png')})
mcp.close()
