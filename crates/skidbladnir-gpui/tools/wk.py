"""Evaluate JavaScript inside the running Tauri window, through WebKitGTK's remote inspector."""
import json, sys, websocket
ws = websocket.create_connection("ws://127.0.0.1:9333/socket/1/1/WebPage", timeout=10)
target = None
for _ in range(20):
    message = json.loads(ws.recv())
    if message.get("method") == "Target.targetCreated":
        target = message["params"]["targetInfo"]["targetId"]
        break
expression = sys.stdin.read()
inner = json.dumps({"id": 1, "method": "Runtime.evaluate", "params": {"expression": expression, "returnByValue": True}})
ws.send(json.dumps({"id": 100, "method": "Target.sendMessageToTarget", "params": {"targetId": target, "message": inner}}))
while True:
    message = json.loads(ws.recv())
    if message.get("method") == "Target.dispatchMessageFromTarget":
        reply = json.loads(message["params"]["message"])
        if reply.get("id") == 1:
            result = reply.get("result", {}).get("result", {})
            print(result.get("value", reply))
            break
