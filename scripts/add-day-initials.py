#!/usr/bin/env python3
"""One-shot helper to inject overview.day.short.{mon..sun} keys into every
locale JSON. Run once; safe to re-run (overwrites if already present)."""
import json, pathlib, sys

DAYS = {
    "en":    ["M",  "T",  "W",  "T",  "F",  "S",  "S"],
    "es":    ["L",  "M",  "X",  "J",  "V",  "S",  "D"],
    "pt-BR": ["S",  "T",  "Q",  "Q",  "S",  "S",  "D"],
    "fr":    ["L",  "M",  "M",  "J",  "V",  "S",  "D"],
    "de":    ["M",  "D",  "M",  "D",  "F",  "S",  "S"],
    "it":    ["L",  "M",  "M",  "G",  "V",  "S",  "D"],
    "ru":    ["П",  "В",  "С",  "Ч",  "П",  "С",  "В"],
    "tr":    ["P",  "S",  "Ç",  "P",  "C",  "C",  "P"],
    "ar":    ["ن",  "ث",  "ر",  "خ",  "ج",  "س",  "ح"],
    "hi":    ["सो", "मं", "बु", "गु", "शु", "श",  "र"],
    "zh-CN": ["一", "二", "三", "四", "五", "六", "日"],
    "ja":    ["月", "火", "水", "木", "金", "土", "日"],
    "ko":    ["월", "화", "수", "목", "금", "토", "일"],
    "id":    ["S",  "S",  "R",  "K",  "J",  "S",  "M"],
}
ORDER = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
ROOT = pathlib.Path(__file__).resolve().parents[1] / "crates" / "i18n" / "locales"

for code, initials in DAYS.items():
    path = ROOT / f"{code}.json"
    if not path.exists():
        print(f"skip missing {path}", file=sys.stderr); continue
    data = json.loads(path.read_text(encoding="utf-8"))
    for k, v in zip(ORDER, initials):
        data[f"overview.day.short.{k}"] = v
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"updated {path.name}")
