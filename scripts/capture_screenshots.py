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
    ("theme_night_lab.png", "?theme=dark"),
    ("theme_daylight.png", "?theme=light"),
    ("theme_classic_dark.png", "?theme=dark"),
    ("theme_classic_light.png", "?theme=light"),
    ("tab_devices.png", "?theme=dark"),
    ("tab_studio.png", "?theme=dark"),
    ("tab_diagnostics.png", "?theme=dark&modal=echo-doctor"),
    ("echo_doctor_modal.png", "?theme=dark&modal=echo-doctor"),
]

for name, query in shots:
    out_file = OUT_DIR / name
    url = f"http://127.0.0.1:{PORT}/{query}"
    print(f"Capturing {name} from {url}...")
    bg_color = "ffeef2f6" if "light" in query else "ff0f172a"

    cmd = [
        str(chrome_path),
        "--headless=new",
        "--disable-gpu",
        "--hide-scrollbars",
        f"--default-background-color={bg_color}",
        "--window-size=460,780",
        "--virtual-time-budget=2000",
        f"--screenshot={out_file}",
        url
    ]
    subprocess.run(cmd, check=True)
    time.sleep(0.3)

print("All screenshots successfully captured!")
