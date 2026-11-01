// Nodejs single file proxy

const http = require("node:http");
const { spawn } = require("child_process");
const path = require("path");

const PROXY_SERVER_PORT = process.env.PORT || 3000;
const APP_SERVER_PORT = 9001;

const appServerBin = path.join(__dirname, "backend-rs");
const appServerArgs = ["serve"];

// 1. start app server
const app = spawn(appServerBin, appServerArgs, {
  env: { ...process.env, NODE_ENV: undefined, PORT: APP_SERVER_PORT },
  stdio: ["ignore", "pipe", "pipe"],
});

app.stdout.on("data", (d) => process.stdout.write("[app] " + d));
app.stderr.on("data", (d) => process.stderr.write("[app] " + d));
app.on("exit", (code) => {
  console.error(`app exited ${code}, stopping Node`);
  process.exit(1); // Passenger will restart us
});

// 2. simple reverse proxy
const server = http.createServer((clientReq, clientRes) => {
  const headers = {
    ...clientReq.headers,
    host: `127.0.0.1:${APP_SERVER_PORT}`,
    "x-forwarded-for": clientReq.socket.remoteAddress,
    "x-forwarded-proto": clientReq.socket.encrypted ? "https" : "http",
  };

  const opts = {
    hostname: "127.0.0.1",
    port: APP_SERVER_PORT,
    path: clientReq.url,
    method: clientReq.method,
    headers,
  };

  const proxy = http.request(opts, (appRes) => {
    clientRes.writeHead(appRes.statusCode, appRes.headers);
    appRes.pipe(clientRes);
  });

  proxy.on("error", () => {
    if (!clientRes.headersSent) {
      clientRes.writeHead(502);
    }
    clientRes.end("Bad gateway");
  });

  clientReq.on("aborted", () => proxy.destroy());
  clientReq.pipe(proxy);
});

// WebSocket support
server.on("upgrade", (req, socket, head) => {
  const headers = {
    ...req.headers,
    host: `127.0.0.1:${APP_SERVER_PORT}`,
  };

  const proxy = http.request({
    hostname: "127.0.0.1",
    port: APP_SERVER_PORT,
    method: req.method,
    path: req.url,
    headers,
  });

  proxy.on("upgrade", (res, proxySocket, proxyHead) => {
    socket.write(`HTTP/${res.httpVersion} 101 Switching Protocols\r\n`);
    for (const [k, v] of Object.entries(res.headers)) {
      socket.write(`${k}: ${v}\r\n`);
    }
    socket.write("\r\n");

    if (proxyHead && proxyHead.length) proxySocket.unshift(proxyHead);
    if (head && head.length) socket.unshift(head);

    proxySocket.pipe(socket).pipe(proxySocket);
  });

  proxy.on("error", () => socket.destroy());
  socket.on("error", () => proxy.destroy());

  proxy.end();
});

server.listen(PROXY_SERVER_PORT, "0.0.0.0", () => {
  console.log("Node proxy listening on", PROXY_SERVER_PORT);
});

// graceful shutdown for Passenger
process.on("SIGTERM", () => {
  server.close(() => app.kill());
});
