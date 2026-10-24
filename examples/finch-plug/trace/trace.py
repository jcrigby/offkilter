"""Measure a pencil tracing on 5-squares-per-inch engineering paper.

    node grab.js flange.jpg masks.json      # classify pixels in Chromium (playwright)
    python3 trace.py masks.json             # grid fit, outline in mm, radius by angle
    node draw.js flange.jpg debug.json grid.png     # the fitted grid over the photo, to check
    node svg2png.js overlay.svg overlay.png         # the outline in mm with the base circle

The grid lines are found by local contrast and fitted scan by scan with a
quadratic (perspective), anchored on one smooth model over the whole
picture; the pencil is whatever is much darker than the paper. The base
circle is fitted on the arcs that are not tabs. A pencil line's centre
sits about half a millimetre outside the part.
"""
import json, math, sys
m = json.load(open(sys.argv[1])); W, H = m["W"], m["H"]; SQ = 25.4 / 5
rows, cols = m["rows"], m["cols"]

def mode_pitch(scans):
    """The grid pitch in pixels: the most common spacing between centres over every scan."""
    from collections import Counter
    cnt = Counter()
    for _, cs in scans:
        for a, b in zip(cs, cs[1:]):
            if 30 < b - a < 150: cnt[round(b - a)] += 1
    p = max(cnt, key=cnt.get)
    near = [(k, n) for k, n in cnt.items() if abs(k - p) <= 4]
    return sum(k * n for k, n in near) / sum(n for _, n in near)

def clean(cs, pitch):
    cs = sorted(cs)
    out = [(cs[0], 0)]
    for c in cs[1:]:
        gap = c - out[-1][0]
        if gap < 0.5 * pitch: continue
        k = round(gap / pitch)
        if abs(gap - k * pitch) > 0.3 * pitch: continue
        out.append((c, out[-1][1] + k))
    return out

def lsq(pairs):
    """fit pos = a + b i + c i^2 by least squares."""
    n = len(pairs); S = [[0.0] * 4 for _ in range(3)]
    for i, x in pairs:
        v = [1.0, i, i * i]
        for r in range(3):
            for c in range(3): S[r][c] += v[r] * v[c]
            S[r][3] += v[r] * x
    for i in range(3):
        p = max(range(i, 3), key=lambda r: abs(S[r][i])); S[i], S[p] = S[p], S[i]
        for r in range(3):
            if r != i:
                f = S[r][i] / S[i][i]
                for c in range(4): S[r][c] -= f * S[i][c]
    return [S[i][3] / S[i][i] for i in range(3)]

def ev(mod, i): return mod[0] + mod[1] * i + mod[2] * i * i

def fit_scans(scans):
    """Per scan a quadratic model pos(i); indices assigned from the neighbouring scan's model.
    The reference scan is the one near the middle whose cleaned lines are most regular."""
    pitch = mode_pitch(scans)
    mid = len(scans) // 2
    def score(j):
        ref = clean(scans[j][1], pitch)
        res = [abs((b - a) - (ib - ia) * pitch) for (a, ia), (b, ib) in zip(ref, ref[1:])]
        return len(ref) - 2 * sum(res) / pitch
    ref_index = max(range(max(0, mid - len(scans) // 4), min(len(scans), mid + len(scans) // 4)), key=score)
    ref = clean(scans[ref_index][1], pitch)
    # choose the index origin so i=0 is near the middle
    mid = ref[len(ref) // 2][1]; ref = [(i - mid, x) for x, i in ref]
    models = {}
    mod = lsq(ref); models[scans[ref_index][0]] = mod
    for direction in (1, -1):
        cur = mod; j = ref_index
        while 0 <= j + direction < len(scans):
            j += direction; coord, cs = scans[j]
            pairs = []
            for c in cs:
                # invert cur: i ≈ (c - a)/b, refine once
                i = (c - cur[0]) / cur[1]; i = i - (ev(cur, i) - c) / (cur[1] + 2 * cur[2] * i)
                k = round(i)
                if abs(ev(cur, k) - c) < 0.22 * pitch: pairs.append((k, c))
            if len(pairs) >= 6:
                new = lsq(pairs)
                # reject outliers and refit
                pairs = [(k, c) for k, c in pairs if abs(ev(new, k) - c) < 4.0]
                if len(pairs) >= 6: cur = lsq(pairs)
            models[coord] = cur
    return pitch, models


def solve(A, b):
    n = len(A[0]); S = [[0.0] * (n + 1) for _ in range(n)]
    for row, y in zip(A, b):
        for r in range(n):
            for c in range(n): S[r][c] += row[r] * row[c]
            S[r][n] += row[r] * y
    for i in range(n):
        p = max(range(i, n), key=lambda r: abs(S[r][i])); S[i], S[p] = S[p], S[i]
        for r in range(n):
            if r != i:
                f = S[r][i] / S[i][i]
                for c in range(n + 1): S[r][c] -= f * S[i][c]
    return [S[i][n] / S[i][i] for i in range(n)]

def basis(t, k): return [1.0, t, t * t, k, k * t, k * k, k * k * t]
def gev(G, t, k): return sum(g * v for g, v in zip(G, basis(t, k)))

def global_fit(scans, models, pitch):
    """pos = f(scan coord t, line index k): one smooth model over the whole picture."""
    G = None
    for it in range(3):
        A = []; b = []
        for t, cs in scans:
            mod = models[t]
            for c in cs:
                if G is None:
                    i = (c - mod[0]) / mod[1]; i = i - (ev(mod, i) - c) / (mod[1] + 2 * mod[2] * i)
                else:
                    i = (c - gev(G, t, 0)) / (G[3] + G[4] * t)
                    for _ in range(3): i -= (gev(G, t, i) - c) / (G[3] + G[4] * t + 2 * (G[5] + G[6] * t) * i)
                k = round(i); pred = ev(mod, k) if G is None else gev(G, t, k)
                if abs(pred - c) < (0.2 * pitch if G is None else 5.0): A.append(basis(t, k)); b.append(c)
        G = solve(A, b)
        res = sorted(abs(gev(G, t, k) - c) for (t, k), c in zip(((row[1], row[3]) for row in A), b))
        print(f"  global fit pass {it}: {len(b)} crossings, residual median {res[len(res)//2]:.2f} px, 90% {res[int(len(res)*0.9)]:.2f}")
    return G
def grid_coord(G, t, pos):
    i = (pos - gev(G, t, 0)) / (G[3] + G[4] * t)
    for _ in range(4): i -= (gev(G, t, i) - pos) / (G[3] + G[4] * t + 2 * (G[5] + G[6] * t) * i)
    return i

def local_refit(scans, G, pitch, half=10):
    out = {}
    for t, cs in scans:
        pairs = []
        for c in cs:
            i = grid_coord(G, t, c); k = round(i)
            if abs(gev(G, t, k) - c) < 4.0: pairs.append((k, c))
        out[t] = lsq(pairs) if len(pairs) >= 8 else None
    ts = [t for t, _ in scans]; sm = {}
    for j, t in enumerate(ts):
        win = [out[u] for u in ts[max(0, j - half): j + half + 1] if out[u] is not None]
        if len(win) < 3: sm[t] = None; continue
        sm[t] = [sorted(w[p] for w in win)[len(win) // 2] for p in range(3)]
    return sm

def coord_local(L, G, t, pos):
    mod = L.get(t)
    if mod is None: return grid_coord(G, t, pos)
    i = (pos - mod[0]) / mod[1]
    for _ in range(3): i -= (ev(mod, i) - pos) / (mod[1] + 2 * mod[2] * i)
    return i

pv, vmod = fit_scans(rows); ph, hmod = fit_scans(cols)
GV = global_fit(rows, vmod, pv); GH = global_fit(cols, hmod, ph)
LV = local_refit(rows, GV, pv); LH = local_refit(cols, GH, ph)
print("pitch px", round(pv, 1), round(ph, 1))
pts = []; dark = m["dark"]
for k in range(0, len(dark), 2):
    x, y = dark[k], dark[k + 1]
    gx = coord_local(LV, GV, y - y % 2, x); gy = coord_local(LH, GH, x - x % 2, y)
    pts.append((gx * SQ, -gy * SQ))
print("mapped", len(pts))

def fit(pts):
    n = len(pts); sx = sy = sxx = syy = sxy = sxxx = syyy = sxyy = sxxy = 0.0
    for x, y in pts:
        sx += x; sy += y; sxx += x*x; syy += y*y; sxy += x*y; sxxx += x**3; syyy += y**3; sxyy += x*y*y; sxxy += x*x*y
    M = [[sxx, sxy, sx, -(sxxx + sxyy)], [sxy, syy, sy, -(sxxy + syyy)], [sx, sy, n, -(sxx + syy)]]
    for i in range(3):
        p = max(range(i, 3), key=lambda r: abs(M[r][i])); M[i], M[p] = M[p], M[i]
        for r in range(3):
            if r != i:
                f = M[r][i] / M[i][i]
                for c in range(4): M[r][c] -= f * M[i][c]
    D, E, F = [M[i][3] / M[i][i] for i in range(3)]
    cx, cy = -D / 2, -E / 2
    return cx, cy, math.sqrt(cx*cx + cy*cy - F)

# first fit on everything within 60 mm of the cloud's centroid (drops stray specks)
mx = sum(x for x, _ in pts) / len(pts); my = sum(y for _, y in pts) / len(pts)
near = [(x, y) for x, y in pts if 25 < math.hypot(x - mx, y - my) < 60]
cx0, cy0, r0 = fit(near)
def profile(cx0, cy0):
    bins = {}
    for x, y in near:
        a = math.degrees(math.atan2(y - cy0, x - cx0)) % 360
        bins.setdefault(int(a // 2) * 2, []).append(math.hypot(x - cx0, y - cy0))
    out = []
    for a in range(0, 360, 2):
        rs = sorted(bins.get(a, [])); out.append((a, rs[len(rs) // 2] if rs else float('nan'), len(rs)))
    return out
for it in range(6):
    prof = profile(cx0, cy0)
    rs = sorted(r for _, r, _ in prof if r == r); floor = rs[len(rs) // 4]   # the lower quartile is the base
    base_angles = {a for a, r, _ in prof if r == r and r < floor + 1.0}
    base = [(x, y) for x, y in near if int((math.degrees(math.atan2(y - cy0, x - cx0)) % 360) // 2) * 2 in base_angles
            and abs(math.hypot(x - cx0, y - cy0) - r0) < 2.0]
    cx0, cy0, r0 = fit(base)
res = sorted(abs(math.hypot(x - cx0, y - cy0) - r0) for x, y in base)
print(f"centre {cx0:.1f},{cy0:.1f}  base radius {r0:.2f} -> OD {2*r0:.1f} mm from {len(base)} pts over {len(base_angles)*2} degrees, residual median {res[len(res)//2]:.2f} 90% {res[int(len(res)*0.9)]:.2f}")
line = profile(cx0, cy0)
for a, r, n in line:
    if a % 4 == 0: print(f"{a:4d} {r:6.2f} {n:3d} {'#' * int(max(0, (r - r0 + 1) * 10)) if r == r else ''}")
json.dump({"centre": [cx0, cy0], "r": r0, "profile": line, "pts": near}, open("profile.json", "w"))
dbg = {"v": [[(c, gev(GV, c, i)) for c in range(0, H, 40)] for i in range(-12, 13)],
       "h": [[(c, gev(GH, c, i)) for c in range(0, W, 40)] for i in range(-16, 17)], "dark": dark}
json.dump(dbg, open("debug.json", "w"))
sv = ['<svg xmlns="http://www.w3.org/2000/svg" viewBox="%f %f 110 110" width="1000" height="1000">' % (cx0 - 55, -cy0 - 55),
      '<rect x="%f" y="%f" width="110" height="110" fill="white"/>' % (cx0 - 55, -cy0 - 55)]
g0 = math.floor((cx0 - 55) / SQ) * SQ; g1 = math.floor((-cy0 - 55) / SQ) * SQ
for i in range(0, 24):
    sv.append('<line x1="%f" y1="%f" x2="%f" y2="%f" stroke="#9cc" stroke-width="0.12"/>' % (g0 + i * SQ, -cy0 - 55, g0 + i * SQ, -cy0 + 55))
    sv.append('<line x1="%f" y1="%f" x2="%f" y2="%f" stroke="#9cc" stroke-width="0.12"/>' % (cx0 - 55, g1 + i * SQ, cx0 + 55, g1 + i * SQ))
sv.append('<circle cx="%f" cy="%f" r="%f" fill="none" stroke="red" stroke-width="0.2"/>' % (cx0, -cy0, r0))
sv.append('<circle cx="%f" cy="%f" r="0.5" fill="red"/>' % (cx0, -cy0))
for x, y in near:
    sv.append('<circle cx="%.2f" cy="%.2f" r="0.1" fill="black"/>' % (x, -y))
for a in range(0, 360, 30):
    sv.append('<text x="%.1f" y="%.1f" font-size="2.5" fill="blue">%d</text>' % (cx0 + 46 * math.cos(math.radians(a)), -cy0 - 46 * math.sin(math.radians(a)), a))
sv.append('</svg>'); open("overlay.svg", "w").write("\n".join(sv))
