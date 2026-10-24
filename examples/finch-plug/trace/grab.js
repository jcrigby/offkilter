const { chromium } = require('playwright'); const fs = require('fs');
(async () => {
  const src = process.argv[2], out = process.argv[3];
  const b = await chromium.launch(); const p = await b.newPage();
  const data = fs.readFileSync(src).toString('base64');
  await p.setContent(`<canvas id=c></canvas>`);
  const res = await p.evaluate(async (data) => {
    const img = new Image(); img.src = 'data:image/jpeg;base64,' + data; await img.decode();
    const c = document.getElementById('c'); c.width = img.width; c.height = img.height;
    const ctx = c.getContext('2d'); ctx.drawImage(img, 0, 0);
    const d = ctx.getImageData(0, 0, c.width, c.height).data;
    const W = c.width, H = c.height;
    // green channel and a 41x41 box mean of it (integral image)
    const g = new Float32Array(W * H), r = new Float32Array(W * H), bl = new Float32Array(W * H);
    for (let i = 0; i < W * H; i++) { r[i] = d[i * 4]; g[i] = d[i * 4 + 1]; bl[i] = d[i * 4 + 2]; }
    const I = new Float64Array((W + 1) * (H + 1));
    for (let y = 1; y <= H; y++) { let s = 0; for (let x = 1; x <= W; x++) { s += g[(y - 1) * W + x - 1]; I[y * (W + 1) + x] = I[(y - 1) * (W + 1) + x] + s; } }
    const R = 20;
    const mean = (x, y) => { const x0 = Math.max(0, x - R), x1 = Math.min(W, x + R + 1), y0 = Math.max(0, y - R), y1 = Math.min(H, y + R + 1);
      return (I[y1 * (W + 1) + x1] - I[y0 * (W + 1) + x1] - I[y1 * (W + 1) + x0] + I[y0 * (W + 1) + x0]) / ((x1 - x0) * (y1 - y0)); };
    const teal = new Uint8Array(W * H); const dark = [];
    for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
      const i = y * W + x, m = mean(x, y);
      if (g[i] < 0.72 * m && Math.abs(r[i] - bl[i]) < 40) dark.push(x, y);
      else if (g[i] < m - 5 && g[i] - r[i] >= -2) teal[i] = 1;
    }
    const rows = [], cols = [];
    for (let y = 0; y < H; y += 2) { const cs = []; let s = -1;
      for (let x = 0; x <= W; x++) { const t = x < W && teal[y * W + x];
        if (t && s < 0) s = x; else if (!t && s >= 0) { if (x - s <= 12) cs.push((s + x - 1) / 2); s = -1; } }
      rows.push([y, cs]); }
    for (let x = 0; x < W; x += 2) { const cs = []; let s = -1;
      for (let y = 0; y <= H; y++) { const t = y < H && teal[y * W + x];
        if (t && s < 0) s = y; else if (!t && s >= 0) { if (y - s <= 12) cs.push((s + y - 1) / 2); s = -1; } }
      cols.push([x, cs]); }
    return { W, H, rows, cols, dark };
  }, data);
  fs.writeFileSync(out, JSON.stringify(res)); console.log(res.W, res.H, 'dark px', res.dark.length / 2); await b.close();
})();
