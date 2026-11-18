import os
import subprocess
import time
import http.server
import socketserver
import threading
from pathlib import Path

PORT = 8189
UI_DIR = Path("crates/quietmic-app/ui").resolve()
OUT_DIR = Path("docs/screenshots").resolve()
OUT_DIR.mkdir(parents=True, exist_ok=True)

class QuietHandler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(UI_DIR), **kwargs)
    def log_message(self, format, *args):
        pass

def run_server():
    with socketserver.TCPServer(("127.0.0.1", PORT), QuietHandler) as httpd:
        httpd.serve_forever()

server_thread = threading.Thread(target=run_server, daemon=True)
server_thread.start()
time.sleep(1)

chrome_path = Path("C:/Program Files/Google/Chrome/Application/chrome.exe")
if not chrome_path.exists():
    chrome_path = Path("C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe")

shots = [
    ("theme_night_lab.png", "?theme=night-lab&tab=tab-lab"),
    ("theme_daylight.png", "?theme=daylight&tab=tab-lab"),
    ("theme_amber_console.png", "?theme=amber-console&tab=tab-lab"),
    ("tab_devices.png", "?theme=night-lab&tab=tab-devices"),
    ("tab_studio.png", "?theme=night-lab&tab=tab-studio"),
    ("tab_diagnostics.png", "?theme=night-lab&tab=tab-diagnostics"),
]

for name, query in shots:
    out_file = OUT_DIR / name
    url = f"http://127.0.0.1:{PORT}/{query}"
    print(f"Capturing {name} from {url}...")
    bg_color = "00000000"
    if "daylight" in query:
        bg_color = "ffeef2f6"
    elif "amber" in query:
        bg_color = "ff0d0b07"
    else:
        bg_color = "ff090a0f"

    cmd = [
        str(chrome_path),
        "--headless=new",
        "--disable-gpu",
        "--hide-scrollbars",
        f"--default-background-color={bg_color}",
        "--window-size=460,700",
        "--virtual-time-budget=2500",
        f"--screenshot={out_file}",
        url
    ]
    subprocess.run(cmd, check=True)
    time.sleep(0.5)

print("All screenshots successfully captured!")
