import sys
import ctypes
import time
import json
import keyboard
from pynput import mouse

def is_admin():
    try:
        return ctypes.windll.shell32.IsUserAnAdmin()
    except Exception:
        return False

if not is_admin():
    ctypes.windll.shell32.ShellExecuteW(
        None, 
        "runas", 
        sys.executable, 
        f'"{sys.argv[0]}"', 
        None, 
        1
    )
    sys.exit()

events = []
start_time = time.time()

def log_event(event_type, **kwargs):
    kwargs["type"] = event_type
    kwargs["elapsed_ms"] = int((time.time() - start_time) * 1000)
    events.append(kwargs)

def on_click(x, y, button, pressed):
    log_event("mouse_down" if pressed else "mouse_up", x=x, y=y, button=str(button))

def on_key(event):
    log_event(f"key_{event.event_type}", key=event.name, scan_code=event.scan_code)

print("Running with Administrator privileges.")
print("Recording started. Click into the game, perform login, then press ESC to finish...")

keyboard.hook(on_key)

with mouse.Listener(on_click=on_click) as m_listener:
    keyboard.wait("esc")
    m_listener.stop()

keyboard.unhook_all()

with open("login_events.json", "w") as f:
    json.dump(events, f, indent=2)

print(f"Finished. Saved {len(events)} events to login_events.json")