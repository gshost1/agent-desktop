const [port, host] = process.argv.slice(2);
const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const page = targets.find((t) => t.type === "page" && t.url.includes(host));
if (!page) {
  console.log(JSON.stringify({ error: "no page on " + host }));
  process.exit(0);
}
const expression = `(() => { const f = document.forms[0]; const fields = {}; for (const [k, v] of new FormData(f)) { if (k in fields) { fields[k] = [].concat(fields[k], v); } else { fields[k] = v; } } if (fields.topping && !Array.isArray(fields.topping)) fields.topping = [fields.topping]; return JSON.stringify({ path: location.pathname, fields }); })()`;
const socket = new WebSocket(page.webSocketDebuggerUrl);
socket.onopen = () => socket.send(JSON.stringify({ id: 1, method: "Runtime.evaluate", params: { expression, returnByValue: true } }));
socket.onmessage = (event) => {
  const message = JSON.parse(event.data);
  if (message.id === 1) {
    console.log(message.result.result.value);
    socket.close();
    process.exit(0);
  }
};
setTimeout(() => { console.log(JSON.stringify({ error: "timeout" })); process.exit(0); }, 10000);
