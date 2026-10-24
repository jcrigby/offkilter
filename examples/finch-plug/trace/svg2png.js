const { chromium } = require('playwright'); const fs = require('fs');
(async () => { const b = await chromium.launch(); const p = await b.newPage({ viewport: { width: 1000, height: 1000 } });
  await p.setContent('<body style="margin:0">' + fs.readFileSync(process.argv[2], 'utf8') + '</body>');
  await p.screenshot({ path: process.argv[3] }); await b.close(); })();
