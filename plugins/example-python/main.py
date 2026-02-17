import json

def on_load(api):
    api.log("Example Python Plugin loaded!")
    try:
        api.show_notice("Hello from Python Plugin!", 5000)
    except Exception as e:
        api.log(f"Failed to show notice: {e}")

def on_unload():
    print("Example Python Plugin unloaded!")

def on_event(event_json):
    # api.log(f"Received event: {event_json}")
    try:
        event = json.loads(event_json)
        event_type = event.get("event_type")
        
        if event_type == "FileSave":
            data = event.get("data", {})
            path = data.get("path", "")
            if path.endswith(".md"):
                print(f"[Python] Markdown file saved: {path}")
    except Exception as e:
        print(f"[Python] Error handling event: {e}")
