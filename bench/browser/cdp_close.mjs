const [port, path] = process.argv.slice(2);
const socket = new WebSocket(`ws://127.0.0.1:${port}${path}`);
const finish = (code) => process.exit(code);
socket.onopen = () => socket.send(JSON.stringify({ id: 1, method: "Browser.close" }));
socket.onmessage = (event) => { if (JSON.parse(event.data).id === 1) finish(0); };
socket.onclose = () => finish(0);
socket.onerror = () => finish(1);
setTimeout(() => finish(2), 8000);
