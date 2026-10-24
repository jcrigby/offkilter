const { chromium } = require('playwright'); const fs = require('fs');
(async () => {
  const b = await chromium.launch(); const p = await b.newPage();
  const data = fs.readFileSync(process.argv[2]).toString('base64');
  const dbg = JSON.parse(fs.readFileSync(process.argv[3]));
  await p.setContent(`<canvas id=c></canvas>`);
  const png = await p.evaluate(async ({data, dbg}) => {
    const img = new Image(); img.src = 'data:image/jpeg;base64,' + data; await img.decode();
    const c = document.getElementById('c'); c.width = img.width; c.height = img.height;
    const ctx = c.getContext('2d'); ctx.drawImage(img, 0, 0);
    ctx.strokeStyle = 'rgba(255,0,0,0.9)'; ctx.lineWidth = 2;
    for (const l of dbg.v) { ctx.beginPath(); l.forEach(([y, x], i) => i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)); ctx.stroke(); }
    ctx.strokeStyle = 'rgba(0,0,255,0.9)';
    for (const l of dbg.h) { ctx.beginPath(); l.forEach(([x, y], i) => i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)); ctx.stroke(); }
    ctx.fillStyle = 'rgba(0,200,0,0.8)';
    for (let i = 0; i < dbg.dark.length; i += 2) ctx.fillRect(dbg.dark[i] - 1, dbg.dark[i + 1] - 1, 3, 3);
    return c.toDataURL('image/png');
  }, {data, dbg});
  fs.writeFileSync(process.argv[4], Buffer.from(png.split(',')[1], 'base64')); await b.close();
})();
