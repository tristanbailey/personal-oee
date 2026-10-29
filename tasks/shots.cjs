// Screenshot baseline: node tasks/shots.cjs [--update] [--no-build]
// Builds the dev site (build_local/), serves it locally, screenshots key pages at mobile and desktop widths,
// then either writes the baseline (--update) or compares against it (default).
const { execSync } = require('child_process');
const fs = require('fs');
const http = require('http');
const path = require('path');
const { chromium } = require('@playwright/test');
const { PNG } = require('pngjs');
const pixelmatch = require('pixelmatch');

const root = path.join(__dirname, '..');
const siteDir = path.join(root, 'build_local');
const baseDir = path.join(root, 'tests', 'screenshots');
const diffDir = path.join(root, '_tmp', 'screenshot-diffs');
const tolerance = 0.001; // share of pixels allowed to differ

const pages = {
    home: '/',
    'blog-index': '/blog/',
    post: '/blog/posts/297-whats-more-exciting-than-an-arduino/',
    casestudy: '/case-studies/crowdbutler/',
    about: '/about/',
    contact: '/contact/',
    '404': '/404.html',
    'about-person': '/about/tristan-bailey/',
    gdpr: '/gdpr/what-is-gdpr/',
    'blog-page-2': '/blog/2/',
    tags: '/tags/',
    search: '/search/',
    '500': '/500/',
};
const widths = { mobile: 390, desktop: 1280 };

const types = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript', '.svg': 'image/svg+xml',
    '.png': 'image/png', '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg', '.gif': 'image/gif', '.webp': 'image/webp',
    '.ico': 'image/x-icon', '.json': 'application/json', '.woff': 'font/woff', '.woff2': 'font/woff2' };

function serve() {
    const server = http.createServer((req, res) => {
        let p = decodeURIComponent(req.url.split('?')[0]);
        let file = path.join(siteDir, p);
        if (!file.startsWith(siteDir)) { res.writeHead(403).end(); return; }
        if (fs.existsSync(file) && fs.statSync(file).isDirectory()) file = path.join(file, 'index.html');
        if (!fs.existsSync(file) && fs.existsSync(file + '.html')) file += '.html';
        if (!fs.existsSync(file)) {
            const notFound = path.join(siteDir, '404', 'index.html');
            res.writeHead(404, { 'Content-Type': 'text/html' });
            res.end(fs.existsSync(notFound) ? fs.readFileSync(notFound) : 'not found');
            return;
        }
        res.writeHead(200, { 'Content-Type': types[path.extname(file)] || 'application/octet-stream' });
        fs.createReadStream(file).pipe(res);
    });
    return new Promise(resolve => server.listen(0, '127.0.0.1', () => resolve(server)));
}

async function main() {
    const update = process.argv.includes('--update');
    if (!process.argv.includes('--no-build')) {
        execSync('pnpm run build', { cwd: root, stdio: 'inherit' });
    }
    const server = await serve();
    const origin = `http://127.0.0.1:${server.address().port}`;
    const browser = await chromium.launch();
    fs.mkdirSync(baseDir, { recursive: true });
    fs.mkdirSync(diffDir, { recursive: true });
    let failures = 0;

    for (const [device, width] of Object.entries(widths)) {
        const context = await browser.newContext({ viewport: { width, height: 900 }, reducedMotion: 'reduce' });
        // Stay deterministic: absolute links to the live domain are served from the local build,
        // every other third-party host is blocked
        await context.route(url => new URL(url).origin !== origin, async route => {
            const u = new URL(route.request().url());
            if (u.hostname === 'holdingbay.co.uk') return route.fulfill({ response: await context.request.get(origin + u.pathname + u.search) });
            return route.abort();
        });
        const page = await context.newPage();
        for (const [name, url] of Object.entries(pages)) {
            const file = `${name}-${device}.png`;
            await page.goto(origin + url, { waitUntil: 'load' });
            await page.addStyleTag({ content: '*,*::before,*::after{animation:none!important;transition:none!important;caret-color:transparent!important}' });
            await page.waitForTimeout(200);
            const shot = await page.screenshot({ fullPage: true });
            const basePath = path.join(baseDir, file);
            if (update || !fs.existsSync(basePath)) {
                fs.writeFileSync(basePath, shot);
                console.log(`wrote  ${file}`);
                continue;
            }
            const a = PNG.sync.read(fs.readFileSync(basePath));
            const b = PNG.sync.read(shot);
            if (a.width !== b.width || a.height !== b.height) {
                console.log(`FAIL   ${file}: size ${a.width}x${a.height} -> ${b.width}x${b.height}`);
                fs.writeFileSync(path.join(diffDir, file), shot);
                failures++;
                continue;
            }
            const diff = new PNG({ width: a.width, height: a.height });
            const changed = pixelmatch(a.data, b.data, diff.data, a.width, a.height, { threshold: 0.1 });
            const ratio = changed / (a.width * a.height);
            if (ratio > tolerance) {
                console.log(`FAIL   ${file}: ${(ratio * 100).toFixed(3)}% pixels differ`);
                fs.writeFileSync(path.join(diffDir, file), PNG.sync.write(diff));
                failures++;
            } else {
                console.log(`ok     ${file} (${(ratio * 100).toFixed(3)}%)`);
            }
        }
        await context.close();
    }
    await browser.close();
    server.close();
    if (failures) console.log(`${failures} screenshot(s) differ; diffs in _tmp/screenshot-diffs/`);
    process.exit(failures ? 1 : 0);
}

main().catch(e => { console.error(e); process.exit(1); });
