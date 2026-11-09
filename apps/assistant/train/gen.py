"""Training data for fine-tuning Needle 3 on the assistant's tools.

Writes `cactus finetune`'s JSONL ({"query", "tools", "answers", "reasoning"}):
phrasings of every command the assistant offers, over many made-up
machines (snapshots: which switches, which apps, what the session allows), so
the model learns to pick from the tools' enums rather than to remember one
machine. Four in ten rows ask for no call, each with a line of reasoning
(which makes it weigh in the loss as much as a call does):

- questions and words about the world, many in a command's words
  ("volume of a sphere formula");
- questions about what's here ("does bluetooth drain the battery");
- a bare name ("night light", "firefox");
- a command taken back ("don't turn on wifi", "never mind");
- a switch or an app this machine hasn't got;
- a command whose tool isn't offered.

The tools come from the daemon itself (`emaki-assistant tools SNAPSHOT`), so
what's trained on is exactly what runs. Nothing leaves the machine: Cactus's
own generator (`cactus generate-data`) would send all this to OpenRouter.

    python3 -I gen.py EMAKI_ASSISTANT_BIN OUT.jsonl [COUNT] [--seed N] [--dev DEV.jsonl]
                      [--paraphrases paraphrases.json]
    python3 -I gen.py --banks BANKS.json

`--banks` writes the phrase lists, each with what it means, for
paraphrase.py to grow on a GPU machine (remote.sh paraphrase); its output,
`--paraphrases`, adds to training's phrasings only: the dev set stays the
hand-written templates kept out, and nothing too close to an eval case is
kept.

About one template in seven is kept out of training for the dev set
(`--dev`: eval cases on the eval's laptop), which picks between runs; the
eval set (../eval/cases.jsonl) is the test, scored once, and no query in it
is trained on.
"""

import difflib
import json
import os
import random
import re
import subprocess
import sys
import tempfile
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))

# ---- the machines --------------------------------------------------------

# (control, title, is a switch)
SWITCHES = [
    ("wifi", "Wi‑Fi", True),
    ("bluetooth", "Bluetooth", True),
    ("focus", "Focus", True),
    ("dark-style", "Dark Style", True),
    ("night-light", "Night Light", True),
    ("airplane", "Airplane Mode", True),
    ("keep-awake", "Keep Awake", True),
    ("vpn", "VPN", True),
    ("tailscale", "Tailscale", True),
    ("wired", "Wired", True),
    ("screenshot", "Screenshot", False),
]
ALWAYS = {"wifi", "bluetooth", "focus", "dark-style", "night-light", "screenshot"}

# (name, kind) — what kind of app, as the phrasings say it
APPS = [
    ("Firefox", "browser"), ("Chromium", "browser"), ("Brave", "browser"),
    ("Google Chrome", "browser"), ("Zen Browser", "browser"),
    ("Ghostty", "terminal"), ("foot", "terminal"), ("Alacritty", "terminal"),
    ("Kitty", "terminal"), ("Console", "terminal"),
    ("Files", "files"), ("Dolphin", "files"), ("Thunar", "files"), ("Nautilus", "files"),
    ("Thunderbird", "email"), ("Geary", "email"), ("Evolution", "email"),
    ("Calculator", "calculator"), ("Qalculate!", "calculator"),
    ("Spotify", "music"), ("Rhythmbox", "music"), ("Amberol", "music"),
    ("Messages", "texts"), ("Signal", "chat"), ("Telegram", "chat"), ("Discord", "chat"),
    ("Visual Studio Code", "code"), ("Zed", "code"), ("VSCodium", "code"),
    ("Text Editor", "notes"), ("Obsidian", "notes"),
    ("Emaki Settings", "settings"),
    ("Image Viewer", "photos"), ("Loupe", "photos"),
    ("Calendar", "calendar"), ("GNOME Calendar", "calendar"),
    ("VLC", "video"), ("mpv", "video"),
    ("Steam", None), ("OBS Studio", None), ("GIMP", None), ("Inkscape", None),
    ("LibreOffice Writer", None), ("LibreOffice Calc", None), ("Slack", None),
    ("Zoom", None), ("Blender", None), ("KeePassXC", None), ("Bitwarden", None),
    ("Krita", None), ("Audacity", None), ("Kdenlive", None), ("Element", None),
]
POWERS = ["lock", "log-out", "sleep", "restart", "power-off"]
# what the session tool calls them (Power::name)
POWER_NAMES = {"lock": "lock", "log-out": "log out", "sleep": "sleep",
               "restart": "restart", "power-off": "power off"}


def machine(rng):
    switches = []
    for control, title, switch in SWITCHES:
        if control in ALWAYS or rng.random() < 0.55:
            switches.append({"control": control, "title": title,
                             "on": rng.random() < 0.5 if switch else None})
    rng.shuffle(switches)
    apps = []
    # one app of most kinds, so "my browser" names one
    by_kind = {}
    for name, kind in APPS:
        by_kind.setdefault(kind, []).append(name)
    for kind, names in by_kind.items():
        if kind is None:
            apps += rng.sample(names, rng.randint(3, 8))
        elif rng.random() < 0.85:
            apps.append(rng.choice(names))
    rng.shuffle(apps)
    # kept under 512 tokens a prompt, which trains twice as fast
    apps = apps[:18]
    powers = [p for p in POWERS if p in ("lock", "log-out") or rng.random() < 0.9]
    return {
        "switches": switches,
        "apps": [{"id": a.lower().replace(" ", "-"), "name": a} for a in apps],
        "power_modes": [],
        "powers": powers,
        "brightness": rng.random() < 0.85,
        "timer_running": False,
    }


def kind_of(name):
    for app, kind in APPS:
        if app == name:
            return kind
    return None


def switch_name(title):
    # Switch::name: normalized title ("Wi‑Fi" → "wifi")
    t = re.sub(r"[-‐‑’']", "", title).lower()
    return " ".join(re.sub(r"[^a-z0-9%:]", " ", t).split())


# ---- the phrasings -------------------------------------------------------
#
# Each phrasing is a template; which templates train and which are kept for
# the dev set is decided by the template's text (see `for_dev`), so the dev
# set asks in ways training never did.

# what people call each switch, for the templates that name it
SWITCH_CALLED = {
    "night-light": ["night light", "night shift", "the blue light filter", "night mode"],
    "dark-style": ["dark mode", "the dark theme", "dark style"],
    "focus": ["do not disturb", "focus mode", "focus"],
    "wifi": ["wifi", "wi-fi", "the wireless", "wlan"],
    "bluetooth": ["bluetooth", "bt"],
    "airplane": ["airplane mode", "flight mode", "plane mode"],
    "keep-awake": ["keep awake", "caffeine mode"],
    "vpn": ["vpn", "the vpn"],
    "tailscale": ["tailscale"],
    "wired": ["ethernet", "the wired connection"],
    "screenshot": ["a screenshot"],
}

SWITCH_ON = ["turn on {x}", "switch on {x}", "enable {x}", "activate {x}", "{x} on",
             "put {x} on", "turn {x} on", "can you turn on {x}", "could you enable {x}",
             "i need {x} on", "get {x} going", "switch {x} on", "start {x}", "{x} on now",
             "go ahead and enable {x}", "i want {x} on"]
SWITCH_OFF = ["turn off {x}", "switch off {x}", "disable {x}", "deactivate {x}", "{x} off",
              "turn {x} off", "shut off {x}", "could you disable {x}", "i don't need {x} anymore",
              "get rid of {x}", "kill {x}", "stop {x}", "no more {x}", "{x} off now",
              "i'm done with {x}, turn it off"]
SWITCH_TOGGLE = ["toggle {x}", "flip {x}"]

# what a switch is asked for without being named, or named in passing
SWITCH_SAY = {
    "night-light": {
        "on": ["make the screen warmer", "warm up the display", "reduce the blue light",
               "enable the blue light filter", "the screen is too blue",
               "my eyes are tired, make the colors warmer", "warmer colors please",
               "give me warmer screen colors", "filter out blue light",
               "it's late, make the screen easier on my eyes", "less blue on the screen",
               "make the display yellower", "evening colors please",
               "the screen is keeping me awake, warm it up", "tint the screen orange",
               "the white is too harsh at night", "switch the display to warm tones",
               "i'm going to bed soon, ease the screen colors"],
        "off": ["make the colors normal again", "the screen looks too orange",
                "remove the blue light filter", "back to normal screen colors",
                "the colors look wrong, too yellow", "i need true colors for photo editing",
                "cool the screen colors back down", "stop tinting the screen",
                "the display is too warm", "it's morning, normal colors"],
    },
    "dark-style": {
        "on": ["use the dark theme", "go dark", "make everything dark", "switch the theme to dark",
               "the white windows are blinding", "make the apps dark",
               "dark colors for the interface", "the light theme hurts my eyes at night",
               "black backgrounds please", "make the desktop darker looking",
               "i prefer dark windows"],
        "off": ["use the light theme", "switch back to light mode", "make the theme light again",
                "light mode please", "i want the bright theme back", "i don't want the dark theme",
                "white windows again", "the dark theme is hard to read in the sun",
                "back to the light look", "go light"],
    },
    "focus": {
        "on": ["don't disturb me", "mute my notifications", "hide notifications for now",
               "i need to focus", "no notifications please", "pause notifications",
               "stop the notifications", "i'm presenting, no popups",
               "silence the alerts for a while", "i don't want to be interrupted",
               "block notifications", "quiet the notifications", "no more popups for now",
               "i'm recording, keep it quiet", "hold all my notifications"],
        "off": ["show notifications again", "i'm done focusing", "let notifications through",
                "notifications back on", "i'm out of the meeting, show alerts again",
                "unmute notifications", "let the popups back", "i want my alerts again",
                "stop blocking notifications"],
    },
    "wifi": {
        "on": ["connect to wireless", "go online", "get me on the internet",
               "connect to the network", "reconnect to the internet", "i need internet",
               "hook me up to the wifi"],
        "off": ["go offline", "disconnect the wireless", "cut the internet connection",
                "drop off the network", "no internet for a while", "take me offline"],
    },
    "bluetooth": {
        "on": ["i want to connect my headphones", "pair my earbuds, turn bluetooth on",
               "my wireless mouse needs bluetooth", "get bluetooth ready for my speaker"],
        "off": ["i'm done with my headphones, bluetooth off", "stop bluetooth to save battery",
                "nothing needs bluetooth now, turn it off"],
    },
    "airplane": {
        "on": ["i'm about to take off", "turn off all the radios", "the plane is taking off",
               "cut every wireless connection", "the flight attendant said airplane mode",
               "boarding now, radios off"],
        "off": ["i landed, turn the radios back on", "leave airplane mode",
                "we landed", "radios back on", "i'm off the plane"],
    },
    "keep-awake": {
        "on": ["keep the screen awake", "stop the computer from sleeping", "stay awake while i read",
               "prevent sleep", "don't let the laptop fall asleep", "never let the screen turn off",
               "keep the display on", "caffeinate", "don't sleep, i'm watching something",
               "the screen keeps going dark while i read", "stop the screen from timing out",
               "don't let it dim while i'm presenting", "keep it on during the download"],
        "off": ["let the screen sleep again", "stop keeping it awake", "allow sleep again",
                "it's fine for the screen to turn off now", "normal sleep again",
                "let it go to sleep on its own"],
    },
    "vpn": {
        "on": ["connect the vpn", "secure my connection", "i'm on public wifi, protect me"],
        "off": ["disconnect vpn", "drop the vpn"],
    },
    "tailscale": {
        "on": ["join my tailnet", "connect to my home network remotely", "get on my tailnet"],
        "off": ["leave the tailnet", "disconnect from my tailnet"],
    },
    "wired": {
        "on": ["use the cable connection", "connect through the cable"],
        "off": ["stop using the cable", "disconnect the cable network"],
    },
    "screenshot": {
        "toggle": ["take a screenshot", "capture the screen", "grab a screenshot",
                   "screenshot please", "snap the screen", "save a picture of my screen",
                   "take a picture of the screen", "capture what's on my display",
                   "i want to save what's on screen", "screen grab", "print screen",
                   "get a screenshot of this"],
    },
}

LEVEL_SAY = {
    ("volume", "up"): ["louder", "turn it up", "crank it up", "boost the sound", "raise the volume",
                       "i can barely hear it", "more sound please", "increase the volume a bit",
                       "pump up the volume", "it's too quiet", "i can't hear the music",
                       "make the sound louder", "volume up", "turn up the sound", "bump the volume",
                       "i can't hear anything", "the video is too quiet", "up the audio",
                       "speak up, it's too soft", "i can hardly hear the call", "more volume",
                       "turn the speakers up", "a little louder", "louder please, i'm cooking"],
    ("volume", "down"): ["quieter", "lower the volume", "it's way too loud", "soften the sound",
                         "turn the music down", "reduce the volume", "too loud",
                         "make it quieter", "volume down a bit", "that's deafening",
                         "turn the sound down a notch", "the baby is sleeping, quieter",
                         "my ears hurt, lower it", "less volume", "tone down the audio",
                         "the speakers are blasting", "turn it down"],
    ("brightness", "up"): ["brighter", "brighten the screen", "the screen is too dim",
                           "more brightness", "increase screen brightness", "the display is too dark",
                           "i can't see the screen well, it's dark", "make the display brighter",
                           "i'm outside and can't read the screen", "light up the screen more",
                           "the screen is hard to see in the sun", "turn up the screen",
                           "boost the brightness", "it's too dark to read"],
    ("brightness", "down"): ["dim the screen", "less bright", "lower the brightness",
                             "the display is blinding", "darken the screen a bit",
                             "the screen hurts my eyes, dim it", "make the display dimmer",
                             "the screen is too bright in this dark room", "tone down the screen",
                             "turn the screen down", "reduce the brightness", "dimmer please",
                             "the display is way too bright"],
}
LEVEL_SET = {
    "volume": ["volume {n}", "set the volume to {n} percent", "sound at {n}%",
               "put the volume at {n}", "volume to {n}", "make the volume {n} percent",
               "change the volume to {n}", "i want the sound at {n}", "audio to {n} percent",
               "set sound level {n}"],
    "brightness": ["brightness {n}", "set screen brightness to {n} percent",
                   "screen at {n}%", "put the brightness at {n}", "brightness to {n}",
                   "make the screen {n} percent bright", "display brightness {n}",
                   "dim the screen to {n} percent", "change brightness to {n}"],
}
LEVEL_WORDS = {
    "volume": [("half volume", 50), ("max volume", 100), ("volume all the way up", 100),
               ("volume to a quarter", 25), ("volume at maximum", 100), ("sound to half", 50),
               ("as loud as it goes", 100), ("lowest volume", 0), ("volume to zero", 0)],
    "brightness": [("full brightness", 100), ("brightness all the way up", 100),
                   ("half brightness", 50), ("brightest the screen goes", 100),
                   ("minimum brightness", 0), ("screen at full", 100), ("brightness to a quarter", 25)],
}

OPEN_SAY = ["open {app}", "launch {app}", "start {app}", "fire up {app}", "bring up {app}",
            "run {app}", "open {app} please", "can you open {app}", "i want to use {app}",
            "get {app} open", "load {app}", "i need {app}", "show me {app}", "switch to {app}",
            "go to {app}", "start up {app} for me", "pull up {app}"]
KIND_SAY = {
    "browser": ["open a browser", "i want to browse the web", "go on the internet",
                "open my web browser", "start the browser", "i need to look something up online",
                "surf the web", "open a new browser window"],
    "terminal": ["open a terminal", "i need a shell", "open the command line", "new terminal",
                 "i want to type some commands", "open a console", "bring up a terminal window"],
    "files": ["show me my files", "open the file manager", "browse my documents",
              "where are my downloads", "open my home folder", "i need to find a file",
              "show my documents folder"],
    "email": ["check my mail", "read my email", "open my inbox", "any new emails",
              "i need to answer an email", "open my mail app", "compose an email"],
    "calculator": ["open the calculator", "calculator please", "i need a calculator app",
                   "i want to do some sums", "bring up the calculator"],
    "music": ["put some music on", "i want to listen to music", "open my music player",
              "i want to hear some songs", "open my music app"],
    "texts": ["send a text", "open my messages", "message someone", "read my texts",
              "i want to text my mom", "check my text messages"],
    "chat": ["open my chat app", "message my friends", "i want to chat with someone"],
    "code": ["start coding", "open my code editor", "open vs code", "launch vscode",
             "i want to program", "open the editor for my project"],
    "notes": ["open my notes", "i want to write something down", "take a note"],
    "settings": ["open settings", "change my settings", "open the settings app",
                 "i want to change my desktop settings", "open system preferences"],
    "photos": ["look at my photos", "open the image viewer", "show my pictures",
               "i want to see my pictures", "view an image"],
    "calendar": ["open my calendar", "what's on my calendar app", "check my schedule app"],
    "video": ["open the video player", "i want to watch a video file"],
}

MEDIA_SAY = {
    "next": ["skip", "next song please", "play the next track", "skip ahead",
             "i don't like this one", "change the song", "skip this track", "next",
             "next one", "move on to the next song", "i'm bored of this song",
             "go to the next track", "skip to the next one", "something else please",
             "forward a track"],
    "previous": ["go back a song", "play the last track again", "previous one",
                 "replay the previous song", "back one track", "go back to the song before",
                 "the song before this one", "previous track", "back to the last song"],
    "pause": ["pause", "hold the music", "stop playing", "pause the video",
              "pause playback", "stop the song", "hold on, pause it", "pause for a second",
              "freeze the video", "stop the playback", "someone's at the door, pause"],
    "play": ["resume", "continue playing", "unpause", "keep playing",
             "start the music again", "play music", "carry on playing", "resume the song",
             "start playing again", "play it", "go on playing"],
}

SESSION_SAY = {
    "lock": ["lock it", "lock my computer", "i'm going for coffee, lock up",
             "secure the screen", "lock the laptop", "lock screen now", "lock the session",
             "i'm leaving my desk, lock it", "lock up please"],
    "log-out": ["log me out", "sign me out", "end my session", "log off", "sign off",
                "log out of my account"],
    "sleep": ["suspend", "go to sleep", "sleep now", "put the computer to sleep",
              "suspend the laptop", "put it in sleep mode", "hibernate for a bit", "nap time for the pc"],
    "restart": ["reboot the machine", "restart now", "do a restart", "reboot please",
                "restart my laptop", "restart the system", "turn it off and on again"],
    "power-off": ["shut it down", "turn the computer off", "power down the laptop",
                  "switch off the pc", "shut down now", "power off", "shut down the system",
                  "i'm done for today, turn it off"],
}

# ---- what isn't a command ------------------------------------------------

# not about this computer at all, many in a command's words
NEGATIVES = [
    "what is wifi", "how does bluetooth work", "why is my screen so bright",
    "how do I take a screenshot", "weather in paris", "is it going to rain tomorrow",
    "capital of australia", "5 + 3", "10 km in miles", "70f to c", "best pizza near me",
    "hello", "thanks", "tell me a story",
    "who are you", "translate hello to french", "write an email to my boss",
    "music theory basics", "songs by queen", "spotify stock price",
    "volume of a sphere formula", "brightness of the sun", "lock screen wallpaper ideas",
    "sleep tips for insomnia", "how long should I sleep", "restart a router guide",
    "what does airplane mode do", "vpn providers comparison", "rust iterator docs",
    "python list comprehension", "how many ounces in a pound", "news today",
    "flights to tokyo", "what's the time in new york", "recipe for pancakes",
    "who wrote hamlet", "define serendipity", "how do I turn off notifications on android",
    "is dark mode better for your eyes", "funny cat videos", "where is my phone",
    "how to make my laptop faster", "terminal velocity of a penny",
    "open source licenses explained", "the office season 3", "next week's schedule",
    "pause in a sentence", "play store refund", "how to lock a door",
    "battery life tips", "what is a screenshot", "louder than words lyrics",
    "skip the line tickets", "power outage map", "restart gpu driver linux command",
    "i'm hungry", "good morning", "what can you do", "ok", "cool", "lol",
    "my cat knocked over my coffee", "remind me what day it is", "how old is the universe",
    "explain quantum computing", "best headphones under 100", "night owl meaning",
    "dark chocolate benefits", "light bulb wattage", "flight status sas 123",
    "focus group questions", "focus on the family", "keep calm and carry on",
    "the wireless router keeps blinking", "my bluetooth speaker is broken",
    "sound of music cast", "brightness of stars", "volume of a cylinder",
    "next train to oslo", "previous president of france", "play chess online",
    "how to pause a netflix download on my phone", "timer for eggs how long",
    "lock and key emoji", "sleep apnea symptoms", "shut down meaning",
    "restart my life", "log in to gmail", "open a bank account", "open sesame",
    "start a business", "launch date of the new phone", "fire up the grill recipes",
    "terminal illness", "files to share with my team", "email etiquette tips",
    "calculator watch", "photos of norway", "settings for a dnd campaign",
    "code of conduct", "messages from the president", "how is the volume of a box measured",
]

# about what's here, without asking to change it
QUESTION_ABOUT = ["how do i turn on {x}", "how does {x} work", "what is {x}", "what does {x} do",
                  "why is {x} not working", "does {x} drain the battery", "is {x} bad for my eyes",
                  "tell me about {x}", "{x} not working", "{x} explained", "best {x} settings",
                  "can i use {x} on a plane", "where is the {x} setting", "history of {x}",
                  "is {x} on", "should i use {x}", "{x} tips", "why did {x} turn off",
                  "{x} keeps turning on by itself", "what's the point of {x}"]
QUESTION_APP = ["is {app} free", "how do i uninstall {app}", "{app} keyboard shortcuts",
                "what is {app}", "{app} won't start", "how to update {app}", "{app} alternatives",
                "is {app} safe", "{app} tutorial", "who makes {app}", "{app} crashed",
                "how much ram does {app} use", "{app} dark theme"]
# just a name
BARE = ["{x}", "the {x}", "{x}?", "{x} setting", "{x} thing"]
BARE_APP = ["{app}", "{app}?", "the {app} app", "{app} app"]
BARE_WORDS = ["volume", "brightness", "sound", "music", "screen", "settings", "media",
              "session", "power", "audio", "display", "song", "track"]
# asking for nothing to happen
NEGATED = ["don't turn on {x}", "don't turn off {x}", "do not enable {x}", "leave {x} alone",
           "don't touch {x}", "no, keep {x} the way it is", "never mind {x}",
           "don't disable {x}", "forget about {x}", "i didn't mean {x}", "no {x} changes",
           "keep {x} as is", "don't mess with {x}"]
NEGATED_APP = ["don't open {app}", "i don't want to open {app}", "never mind {app}",
               "no, not {app}", "don't launch {app}"]
NEGATED_PLAIN = ["don't change the volume", "leave the brightness as it is", "don't skip this song",
                 "don't lock the screen", "don't restart", "don't shut down", "don't pause it",
                 "keep the volume where it is", "don't log me out", "don't put it to sleep",
                 "no, don't do that", "never mind", "cancel that", "forget it", "nothing, sorry",
                 "i changed my mind", "wait, no", "don't turn it up"]


# ---- paraphrases ---------------------------------------------------------

# what each switch's on and off is, in words a paraphrase can keep to
SWITCH_MEANS = {
    ("night-light", "on"): "turn the night light on (warmer, less blue screen colors)",
    ("night-light", "off"): "turn the night light off (back to normal screen colors)",
    ("dark-style", "on"): "switch to the dark theme",
    ("dark-style", "off"): "switch dark mode off, back to the light theme",
    ("focus", "on"): "turn do not disturb on (silence notifications)",
    ("focus", "off"): "turn do not disturb off (let notifications through)",
    ("wifi", "on"): "turn wi-fi on", ("wifi", "off"): "turn wi-fi off",
    ("bluetooth", "on"): "turn bluetooth on", ("bluetooth", "off"): "turn bluetooth off",
    ("airplane", "on"): "turn airplane mode on", ("airplane", "off"): "turn airplane mode off",
    ("keep-awake", "on"): "keep the computer from sleeping or the screen from turning off",
    ("keep-awake", "off"): "let the computer sleep normally again",
    ("vpn", "on"): "connect the vpn", ("vpn", "off"): "disconnect the vpn",
    ("tailscale", "on"): "connect tailscale", ("tailscale", "off"): "disconnect tailscale",
    ("wired", "on"): "connect the wired ethernet connection",
    ("wired", "off"): "disconnect the wired ethernet connection",
    ("screenshot", "toggle"): "take a screenshot",
}
LEVEL_MEANS = {
    ("volume", "up"): "make the sound louder", ("volume", "down"): "make the sound quieter",
    ("brightness", "up"): "make the screen brighter", ("brightness", "down"): "make the screen dimmer",
}
KIND_MEANS = {
    "browser": "a web browser", "terminal": "a terminal", "files": "the file manager",
    "email": "the email app", "calculator": "the calculator", "music": "the music player",
    "texts": "the text messages app", "chat": "a chat app", "code": "the code editor",
    "notes": "the notes app", "settings": "the system settings", "photos": "the photo viewer",
    "calendar": "the calendar", "video": "the video player",
}
MEDIA_MEANS = {
    "next": "skip to the next song or track", "previous": "go back to the previous song or track",
    "pause": "pause the music or video that's playing", "play": "resume or start playing music",
}
SESSION_MEANS = {
    "lock": "lock the screen", "log-out": "log out of the session",
    "sleep": "put the computer to sleep", "restart": "restart the computer",
    "power-off": "shut the computer down",
}
ANY_COMMAND = ("Is the person asking the computer itself to do something right now: change a "
               "setting, open an app, control the music or video, or lock, sleep, restart or "
               "shut it down?")


def banks():
    """Every phrase list a paraphrase can grow: (key, phrases, meaning,
    check, want). The meaning is what all its phrases ask; the check a yes or
    no question a new phrasing has to answer `want` to, with {x} as
    bluetooth, {app} as Firefox and {n} as 40."""
    out = []

    def add(key, phrases, meaning, check, want="yes"):
        out.append((key, phrases, meaning, check, want))

    for control, says in SWITCH_SAY.items():
        for state, phrases in says.items():
            means = SWITCH_MEANS[(control, state)]
            add(f"switch/{control}/{state}", phrases, f"asking the computer to {means}",
                f"Is the person asking the computer to {means}?")
    for state, templates in (("on", SWITCH_ON), ("off", SWITCH_OFF), ("toggle", SWITCH_TOGGLE)):
        add(f"switch-named/{state}", templates,
            f"asking the computer to {'toggle' if state == 'toggle' else 'turn ' + state} "
            "the setting named {x}",
            f"Is the person asking the computer to {'toggle' if state == 'toggle' else 'turn ' + state} bluetooth?")
    for (what, change), phrases in LEVEL_SAY.items():
        means = LEVEL_MEANS[(what, change)]
        add(f"level/{what}/{change}", phrases, f"asking the computer to {means}",
            f"Is the person asking the computer to {means}?")
    for what, templates in LEVEL_SET.items():
        add(f"level-set/{what}", templates, f"asking to set the {what} to {{n}} percent",
            f"Is the person asking the computer to set the {what} to 40 percent?")
    add("open", OPEN_SAY, "asking the computer to open the app {app}",
        "Is the person asking the computer to open Firefox?")
    for kind, phrases in KIND_SAY.items():
        add(f"kind/{kind}", phrases, f"asking the computer to open {KIND_MEANS[kind]}",
            f"Is the person asking the computer to open {KIND_MEANS[kind]}?")
    for command, phrases in MEDIA_SAY.items():
        add(f"media/{command}", phrases, f"asking the computer to {MEDIA_MEANS[command]}",
            f"Is the person asking the computer to {MEDIA_MEANS[command]}?")
    for power, phrases in SESSION_SAY.items():
        add(f"session/{power}", phrases, f"asking the computer to {SESSION_MEANS[power]}",
            f"Is the person asking the computer to {SESSION_MEANS[power]}?")

    add("none/general", NEGATIVES,
        "a search, question or remark that isn't a command to this computer, often in a "
        "command's words used another way (\"volume of a sphere\", \"play store refund\", "
        "\"lock and key emoji\", \"restart my life\")", ANY_COMMAND, "no")
    add("none/question", QUESTION_ABOUT,
        "a question or remark about the setting {x} that doesn't ask to change it",
        "Is the person asking the computer to turn bluetooth on or off right now?", "no")
    add("none/question-app", QUESTION_APP,
        "a question or remark about the app {app} that doesn't ask to open it",
        "Is the person asking the computer to open Firefox right now?", "no")
    add("none/negated", NEGATED,
        "asking for the setting {x} to be left as it is, or taking back a request about it",
        "Is the person asking the computer to change bluetooth (turn it on, off, or toggle it)?", "no")
    add("none/negated-app", NEGATED_APP, "asking the computer not to open the app {app}",
        "Is the person asking the computer to open Firefox?", "no")
    add("none/negated-plain", NEGATED_PLAIN,
        "asking the computer not to change something (the volume, brightness, music, the "
        "session), or taking back what was just asked", ANY_COMMAND, "no")
    return out


# phrasings that came from paraphrases: training's only, never the dev set's
PARAPHRASED = set()


def near(a, b):
    return difflib.SequenceMatcher(None, a, b).ratio() >= 0.9


def add_paraphrases(path, held):
    """Grows the phrase lists from `path` (paraphrase.py's output). A
    phrasing too close to an eval case or to a dev template is dropped, so
    neither is trained on in other words."""
    with open(path) as f:
        grown = json.load(f)
    lists = {key: phrases for key, phrases, *_ in banks()}
    dev = [plain(p) for phrases in lists.values() for p in phrases if for_dev(p)]
    added = dropped = 0
    for key, phrases in grown.items():
        target = lists.get(key)
        if target is None:
            continue
        have = {plain(p) for p in target}
        for phrase in phrases:
            # its placeholders are the list's, and nothing else is in braces
            if re.sub(r"\{(x|app|n)\}", "", phrase).count("{") or "}" in re.sub(r"\{(x|app|n)\}", "", phrase):
                continue
            words = plain(phrase)
            if words in have:
                continue
            if any(near(words, h) for h in held) or any(near(words, d) for d in dev):
                dropped += 1
                continue
            have.add(words)
            target.append(phrase)
            PARAPHRASED.add(phrase)
            added += 1
    return added, dropped


def for_dev(template):
    """Whether a template is the dev set's: about one in seven, by its text."""
    if template in PARAPHRASED:
        return False
    return zlib.crc32(template.encode()) % 7 == 0


def positives(rng, snapshot):
    """Every command phrasing this machine can be asked, as (template,
    query, answers, reasoning)."""
    out = []
    names = {s["control"]: switch_name(s["title"]) for s in snapshot["switches"]}

    def switch(template, q, control, state):
        out.append((template, q, [{"name": "set_switch",
                                   "arguments": {"setting": names[control], "state": state}}],
                    f"{names[control]} {state}"))

    for control, says in SWITCH_SAY.items():
        if control not in names:
            continue
        for state, phrases in says.items():
            for q in phrases:
                switch(q, q, control, state)
    for control, called in SWITCH_CALLED.items():
        if control not in names or control == "screenshot":
            continue
        for state, templates in (("on", SWITCH_ON), ("off", SWITCH_OFF), ("toggle", SWITCH_TOGGLE)):
            for t in rng.sample(templates, min(4, len(templates))):
                switch(t, t.format(x=rng.choice(called)), control, state)

    for (what, change), phrases in LEVEL_SAY.items():
        if what == "brightness" and not snapshot["brightness"]:
            continue
        for q in phrases:
            direction = {"up": "increase", "down": "decrease"}[change]
            out.append((q, q, [{"name": "change_level", "arguments": {"what": what, "direction": direction}}],
                        f"{what} {change}"))
    for what, templates in LEVEL_SET.items():
        if what == "brightness" and not snapshot["brightness"]:
            continue
        for t in templates:
            n = rng.choice([5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 75, 80, 85, 90, 95])
            out.append((t, t.format(n=n),
                        [{"name": "set_level", "arguments": {"what": what, "percent": n}}],
                        f"{what} {n}"))
        for q, n in LEVEL_WORDS[what]:
            out.append((q, q, [{"name": "set_level", "arguments": {"what": what, "percent": n}}],
                        f"{what} {n}"))

    apps = [a["name"] for a in snapshot["apps"]]
    for app in rng.sample(apps, min(6, len(apps))):
        for t in rng.sample(OPEN_SAY, 3):
            q = t.format(app=app.lower() if rng.random() < 0.5 else app)
            out.append((t, q, [{"name": "open_app", "arguments": {"name": app}}], f"open {app}"))
    kinds = {}
    for app in apps:
        kinds.setdefault(kind_of(app), []).append(app)
    for kind, phrases in KIND_SAY.items():
        if len(kinds.get(kind, [])) != 1:
            continue
        app = kinds[kind][0]
        for q in phrases:
            out.append((q, q, [{"name": "open_app", "arguments": {"name": app}}],
                        f"the {kind} app is {app}"))

    for command, phrases in MEDIA_SAY.items():
        for q in phrases:
            out.append((q, q, [{"name": "media", "arguments": {"command": command}}],
                        f"media {command}"))
    for power, phrases in SESSION_SAY.items():
        if power not in snapshot["powers"]:
            continue
        for q in phrases:
            out.append((q, q, [{"name": "session", "arguments": {"action": POWER_NAMES[power]}}],
                        f"session {POWER_NAMES[power]}"))
    return out


def negatives(rng, snapshot, kind):
    """One phrasing of `kind` that asks for no call, as (template, query,
    reasoning), or None when this machine has nothing for it."""
    here = [c for c in SWITCH_CALLED if c in {s["control"] for s in snapshot["switches"]}
            and c != "screenshot"]
    apps = [a["name"] for a in snapshot["apps"]]
    if kind == "general":
        q = rng.choice(NEGATIVES)
        return q, q, rng.choice(["not something this computer does", "no tool for this",
                                 "a question for the web, not a command"])
    if kind == "question":
        if apps and rng.random() < 0.35:
            t, app = rng.choice(QUESTION_APP), rng.choice(apps)
            return t, t.format(app=app.lower()), f"asks about {app}, doesn't ask to open it"
        if not here:
            return None
        t, x = rng.choice(QUESTION_ABOUT), rng.choice(SWITCH_CALLED[rng.choice(here)])
        return t, t.format(x=x), f"asks about {x}, doesn't ask to change it"
    if kind == "bare":
        r = rng.random()
        if r < 0.3:
            q = rng.choice(BARE_WORDS)
            return q, q, f"just the word {q}"
        if r < 0.6 and apps:
            t, app = rng.choice(BARE_APP), rng.choice(apps)
            return t, t.format(app=app.lower() if rng.random() < 0.6 else app), f"just names {app}"
        if not here:
            return None
        t, x = rng.choice(BARE), rng.choice(SWITCH_CALLED[rng.choice(here)])
        return t, t.format(x=x), f"just names {x}"
    if kind == "negated":
        r = rng.random()
        if r < 0.3:
            q = rng.choice(NEGATED_PLAIN)
            return q, q, "asks for nothing to change"
        if r < 0.5 and apps:
            t, app = rng.choice(NEGATED_APP), rng.choice(apps)
            return t, t.format(app=app.lower()), f"asks not to open {app}"
        if not here:
            return None
        t, x = rng.choice(NEGATED), rng.choice(SWITCH_CALLED[rng.choice(here)])
        return t, t.format(x=x), f"asks to leave {x} alone"
    if kind == "absent":
        controls = {s["control"] for s in snapshot["switches"]}
        # vpn and tailscale are both "a vpn": one standing in for the other isn't wrong
        gone = [c for c in SWITCH_CALLED if c not in controls and c != "screenshot"
                and not (c in ("vpn", "tailscale") and {"vpn", "tailscale"} & controls)]
        if rng.random() < 0.5:
            missing = [name for name, _ in APPS if name not in apps]
            app = rng.choice(missing)
            t = rng.choice(OPEN_SAY)
            return t, t.format(app=app.lower()), f"there is no {app} here"
        if not gone:
            return None
        control = rng.choice(gone)
        t = rng.choice(SWITCH_ON + SWITCH_OFF)
        x = rng.choice(SWITCH_CALLED[control])
        return t, t.format(x=x), f"there is no {x.removeprefix('the ')} here"
    raise ValueError(kind)


# of every hundred rows, how many of each
MIX = {
    # the commands, about as often as each is asked
    "set_switch": 18, "change_level": 9, "set_level": 5, "open_app": 12, "media": 8,
    # the session is the rules' alone: no tool is offered for it, so its
    # words ask for no call (the dev set still expects the rules' command)
    "session": 6,
    # and what isn't one
    "general": 10, "question": 7, "bare": 4, "negated": 8, "absent": 7,
    # a command whose tool isn't offered
    "removed": 6,
}


def vary(rng, q):
    """A little of how people type: case, punctuation, a polite word, a slip."""
    r = rng.random()
    if r < 0.12:
        q = "please " + q
    elif r < 0.22:
        q = q + " please"
    elif r < 0.28:
        q = "hey, " + q
    elif r < 0.33:
        q = "can you " + q
    if rng.random() < 0.3:
        q = q[0].upper() + q[1:]
    if rng.random() < 0.15 and not q.endswith("?"):
        q += rng.choice([".", "!"])
    if rng.random() < 0.04:
        # two letters swapped, as a hurried hand does
        words = q.split(" ")
        i = rng.randrange(len(words))
        w = words[i]
        if len(w) > 3:
            j = rng.randrange(len(w) - 1)
            words[i] = w[:j] + w[j + 1] + w[j] + w[j + 2:]
        q = " ".join(words)
    return q


def plain(q):
    return " ".join(re.sub(r"[^a-z0-9% ]", " ", q.lower().replace("'", "")).split())


def expect(answers, snapshot):
    """The command the eval expects for `answers` on `snapshot`, or None."""
    if not answers:
        return None
    call = answers[0]
    name, args = call["name"], call["arguments"]
    if name == "set_switch":
        for s in snapshot["switches"]:
            if switch_name(s["title"]) == args["setting"]:
                state = "toggle" if s["on"] is None else args["state"]
                return {"do": "switch", "control": s["control"], "state": state}
    if name == "set_level":
        return {"do": "level", "level": args["what"], "target": {"percent": args["percent"]}}
    if name == "change_level":
        target = {"increase": "up", "decrease": "down"}[args["direction"]]
        return {"do": "level", "level": args["what"], "target": target}
    if name == "open_app":
        for a in snapshot["apps"]:
            if a["name"] == args["name"]:
                return {"do": "open", "app": a["id"]}
    if name == "media":
        return {"do": "media", "media": args["command"]}
    if name == "session":
        power = {v: k for k, v in POWER_NAMES.items()}[args["action"]]
        return {"do": "power", "power": power}
    raise ValueError(call)


def tools_for(binary, snapshot, tmp, i):
    path = os.path.join(tmp, f"{i}.json")
    with open(path, "w") as f:
        json.dump(snapshot, f)
    tools = json.loads(subprocess.run([binary, "tools", path], check=True,
                                      capture_output=True, text=True).stdout)
    # Needle's compact form: the function itself
    return [t["function"] for t in tools]


def rows_for(rng, machines, count, dev, held):
    """`count` training rows (or, with `dev`, rows from the dev templates)."""
    kinds, weights = list(MIX), list(MIX.values())
    rows, seen, tries = [], set(), 0
    while len(rows) < count and tries < count * 50:
        tries += 1
        index = rng.randrange(len(machines))
        snapshot, tools = machines[index]
        kind = rng.choices(kinds, weights=weights)[0]
        offered = tools
        if kind in ("set_switch", "change_level", "set_level", "open_app", "media", "session",
                    "removed"):
            found = positives(rng, snapshot)
            if kind == "removed":
                found = [f for f in found if f[2][0]["name"] != "session"]
                template, q, answers, _ = rng.choice(found)
                offered = [t for t in tools if t["name"] != answers[0]["name"]]
                answers, reasoning = [], f"nothing offered for {answers[0]['name'].replace('_', ' ')}"
            else:
                found = [f for f in found if f[2][0]["name"] == kind]
                if not found:
                    continue
                template, q, answers, reasoning = rng.choice(found)
                if kind == "session" and not dev:
                    answers, reasoning = [], "the session isn't offered"
        else:
            got = negatives(rng, snapshot, kind)
            if got is None:
                continue
            template, q, reasoning = got
            answers = []
        if for_dev(template) != dev:
            continue
        q = vary(rng, q)
        # the same words on another machine is another example: its tools differ
        key = (plain(q), json.dumps(answers, sort_keys=True), index if not dev else 0)
        if plain(q) in held or key in seen:
            continue
        seen.add(key)
        rows.append((index, {"query": q, "tools": offered, "answers": answers,
                             "reasoning": reasoning}))
    return rows


def main():
    args = sys.argv[1:]

    def option(name, default=None):
        if name in args:
            i = args.index(name)
            value = args[i + 1]
            del args[i:i + 2]
            return value
        return default

    seed = int(option("--seed", "0"))
    dev_path = option("--dev")
    paraphrases = option("--paraphrases")
    banks_path = option("--banks")
    if banks_path:
        # what paraphrase.py is asked to grow: the training phrasings only
        with open(banks_path, "w") as f:
            json.dump([{"key": key, "phrases": [p for p in phrases if not for_dev(p)],
                        "meaning": meaning, "check": check, "want": want}
                       for key, phrases, meaning, check, want in banks()], f, indent=1)
        print(f"{len(banks())} phrase lists -> {banks_path}")
        return
    binary, out_path = args[0], args[1]
    count = int(args[2]) if len(args) > 2 else 20000
    rng = random.Random(seed)

    held = set()
    with open(os.path.join(HERE, "..", "eval", "cases.jsonl")) as cases:
        for line in cases:
            if line.strip():
                held.add(plain(json.loads(line)["text"]))
    if paraphrases:
        added, dropped = add_paraphrases(paraphrases, held)
        print(f"{added} paraphrased phrasings ({dropped} too close to the eval or dev set)")

    with tempfile.TemporaryDirectory() as tmp:
        machines = []
        for i in range(200):
            snapshot = machine(rng)
            machines.append((snapshot, tools_for(binary, snapshot, tmp, i)))
        with open(os.path.join(HERE, "..", "eval", "laptop.json")) as f:
            laptop = json.load(f)
        laptop_tools = tools_for(binary, laptop, tmp, "laptop")

    rows = [row for _, row in rows_for(rng, machines, count, False, held)]
    with open(out_path, "w") as f:
        for row in rows:
            f.write(json.dumps(row, ensure_ascii=False) + "\n")
    calls = sum(1 for r in rows if r["answers"])
    print(f"{len(rows)} examples ({calls} calls, {len(rows) - calls} none) from "
          f"{len(machines)} machines -> {out_path}")

    if dev_path:
        # the dev set asks the eval's laptop, in the dev templates' words; a
        # tool taken away isn't something the eval can ask
        mix = dict(MIX)
        MIX["removed"] = 0
        dev_rows = rows_for(random.Random(seed + 1), [(laptop_snapshot(laptop), laptop_tools)],
                            400, True, held)
        MIX.update(mix)
        written = set()
        with open(dev_path, "w") as f:
            for _, row in dev_rows:
                text = row["query"]
                if plain(text) in written:
                    continue
                written.add(plain(text))
                case = {"text": text, "expect": expect(row["answers"], laptop)}
                f.write(json.dumps(case, ensure_ascii=False) + "\n")
        print(f"{len(written)} dev cases -> {dev_path}")


def laptop_snapshot(laptop):
    """The eval's laptop as gen.py's machines are: apps by name, with the
    kinds the templates know them by."""
    snapshot = dict(laptop)
    snapshot["apps"] = [{"id": a["id"], "name": a["name"]} for a in laptop["apps"]]
    return snapshot


if __name__ == "__main__":
    main()
