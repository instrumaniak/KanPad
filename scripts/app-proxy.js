const http = require('node:http');
const { spawn } = require('node:child_process');
const path = require('node:path');
const net = require('node:net');

const PROXY_SERVER_PORT = process.env.PORT || 3000;
const appServerBin = path.join(__dirname, 'backend-rs');
const appServerArgs = ['serve'];

let app = null;

// Helper to reliably find a completely free random local port
function findFreePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.unref();
    server.on('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });
}

async function startServer() {
  const APP_SERVER_PORT = await findFreePort().catch(() => 9005);

  console.log(`[proxy] Spawning Rust backend on dynamic port: ${APP_SERVER_PORT}`);

  // Spawn Rust child process directly under the Node process group
  app = spawn(appServerBin, appServerArgs, {
    env: { ...process.env, NODE_ENV: undefined, PORT: APP_SERVER_PORT },
    stdio: ['ignore', 'pipe', 'pipe'],
    detached: false, // Ensures child dies if parent gets SIGKILL'd
  });

  app.stdout.on('data', (d) => process.stdout.write('[app] ' + d));
  app.stderr.on('data', (d) => process.stderr.write('[app] ' + d));

  app.on('exit', (code) => {
    console.error(`[proxy] Rust backend exited with code ${code}. Stopping Node wrapper.`);
    process.exit(code || 1);
  });

  // Create reverse proxy
  const server = http.createServer((clientReq, clientRes) => {
    const headers = {
      ...clientReq.headers,
      host: `127.0.0.1:${APP_SERVER_PORT}`,
      'x-forwarded-for': clientReq.socket.remoteAddress,
      'x-forwarded-proto': clientReq.socket.encrypted ? 'https' : 'http',
    };

    const proxy = http.request(
      {
        hostname: '127.0.0.1',
        port: APP_SERVER_PORT,
        path: clientReq.url,
        method: clientReq.method,
        headers,
      },
      (appRes) => {
        clientRes.writeHead(appRes.statusCode, appRes.headers);
        appRes.pipe(clientRes);
      },
    );

    proxy.on('error', () => {
      if (!clientRes.headersSent) clientRes.writeHead(502);
      clientRes.end('Bad gateway');
    });

    clientReq.on('aborted', () => proxy.destroy());
    clientReq.pipe(proxy);
  });

  // WebSocket support
  server.on('upgrade', (req, socket, head) => {
    const proxy = http.request({
      hostname: '127.0.0.1',
      port: APP_SERVER_PORT,
      method: req.method,
      path: req.url,
      headers: { ...req.headers, host: `127.0.0.1:${APP_SERVER_PORT}` },
    });

    proxy.on('upgrade', (res, proxySocket, proxyHead) => {
      socket.write(`HTTP/${res.httpVersion} 101 Switching Protocols\r\n`);
      for (const [k, v] of Object.entries(res.headers)) {
        socket.write(`${k}: ${v}\r\n`);
      }
      socket.write('\r\n');

      if (proxyHead?.length) proxySocket.unshift(proxyHead);
      if (head?.length) socket.unshift(head);

      proxySocket.pipe(socket).pipe(proxySocket);
    });

    proxy.on('error', () => socket.destroy());
    socket.on('error', () => proxy.destroy());
    proxy.end();
  });

  server.listen(PROXY_SERVER_PORT, '0.0.0.0', () => {
    console.log('[proxy] Node wrapper listening on external port:', PROXY_SERVER_PORT);
  });
}

// --- ABSOLUTE LIFECYCLE CLEANUP ---
const cleanup = () => {
  if (app && !app.killed) {
    console.log('[proxy] Forcefully terminating child Rust process...');
    app.kill('SIGKILL'); // SIGKILL prevents Rust from hanging on long connections
  }
};

// Handle all shutdown angles from cPanel/LiteSpeed
process.on('SIGTERM', () => {
  cleanup();
  process.exit(0);
});
process.on('SIGINT', () => {
  cleanup();
  process.exit(0);
});
process.on('exit', () => {
  cleanup();
});
process.on('uncaughtException', (err) => {
  console.error('[proxy] Uncaught Exception encountered:', err);
  cleanup();
  process.exit(1);
});

// Fire initialization
startServer();
