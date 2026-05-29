#!/usr/bin/env node
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');
const http = require('http');

async function main() {
    const args = parseArgs(process.argv.slice(2));
    const { input, output, viewport, diagnosticsPath, clipViewport, servePort } = args;

    if (!input || !output) {
        console.error('Usage: node screenshot.js --input=path.html --output=path.png [--viewport=1600x900] [--diagnostics=path.json] [--clip-viewport] [--serve-port=PORT]');
        process.exit(1);
    }

    const [width, height] = viewport.split('x').map(Number);

    // Start a tiny HTTP server to serve the HTML file (avoids file:// restrictions)
    let server = null;
    let serverUrl = null;
    if (servePort) {
        const inputDir = path.dirname(path.resolve(input));
        server = http.createServer((req, res) => {
            const filePath = path.join(inputDir, req.url === '/' ? 'index.html' : req.url);
            fs.readFile(filePath, (err, data) => {
                if (err) {
                    res.writeHead(404);
                    res.end('Not found');
                    return;
                }
                const ext = path.extname(filePath);
                const contentType = {
                    '.html': 'text/html',
                    '.js': 'application/javascript',
                    '.css': 'text/css',
                    '.png': 'image/png',
                    '.jpg': 'image/jpeg',
                    '.jpeg': 'image/jpeg',
                    '.svg': 'image/svg+xml',
                }[ext] || 'application/octet-stream';
                res.writeHead(200, { 'Content-Type': contentType });
                res.end(data);
            });
        });
        await new Promise((resolve) => server.listen(servePort, '127.0.0.1', resolve));
        serverUrl = `http://127.0.0.1:${servePort}/index.html`;
    }

    const browser = await chromium.launch({ headless: true });
    const page = await browser.newPage({ viewport: { width, height } });

    const consoleErrors = [];
    const pageErrors = [];
    const warnings = [];

    page.on('console', msg => {
        if (msg.type() === 'error') consoleErrors.push(msg.text());
        else if (msg.type() === 'warning') warnings.push(msg.text());
    });

    page.on('pageerror', err => {
        pageErrors.push(err.message);
    });

    try {
        const url = serverUrl || `file://${path.resolve(input)}`;
        await page.goto(url, { waitUntil: 'networkidle' });

        const screenshotOptions = { path: output, type: 'png' };
        if (clipViewport) {
            screenshotOptions.clip = { x: 0, y: 0, width, height };
        } else {
            screenshotOptions.fullPage = true;
        }

        await page.screenshot(screenshotOptions);

        if (diagnosticsPath) {
            fs.writeFileSync(diagnosticsPath, JSON.stringify({
                consoleErrors,
                pageErrors,
                warnings,
                viewport: { width, height },
            }, null, 2));
        }

        process.exit(0);
    } catch (e) {
        console.error('Screenshot failed:', e.message);
        if (diagnosticsPath) {
            fs.writeFileSync(diagnosticsPath, JSON.stringify({
                consoleErrors,
                pageErrors,
                warnings,
                error: e.message,
            }, null, 2));
        }
        process.exit(1);
    } finally {
        await browser.close();
        if (server) {
            server.close();
        }
    }
}

function parseArgs(argv) {
    const args = {};
    for (const arg of argv) {
        const [key, value] = arg.split('=');
        const cleanKey = key.replace(/^--/, '');
        args[cleanKey] = value !== undefined ? value : true;
    }
    return args;
}

main();
