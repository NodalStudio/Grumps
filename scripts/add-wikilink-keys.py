#!/usr/bin/env python3
"""Localize wikilink/backlink UI strings."""
import json, pathlib

KEYS = {
    "page.note_editor.backlinks_heading": {
        "en": "Linked from", "es": "Enlazado desde", "pt-BR": "Vinculado de",
        "fr": "Lié depuis", "de": "Verlinkt von", "it": "Collegato da",
        "ru": "Ссылаются", "tr": "Bağlantı verenler", "ar": "مرتبط من",
        "hi": "यहाँ से लिंक", "zh-CN": "被链接自", "ja": "リンク元",
        "ko": "링크한 노트", "id": "Ditautkan dari",
    },
    "page.note_editor.wikilink_create": {
        "en": "Create note “{title}”?", "es": "¿Crear nota «{title}»?",
        "pt-BR": "Criar nota “{title}”?", "fr": "Créer la note « {title} » ?",
        "de": "Notiz „{title}“ erstellen?", "it": "Creare la nota «{title}»?",
        "ru": "Создать заметку «{title}»?", "tr": "“{title}” notu oluşturulsun mu?",
        "ar": "إنشاء ملاحظة ‏«{title}»؟", "hi": "नोट “{title}” बनाएँ?",
        "zh-CN": "创建笔记“{title}”?", "ja": "ノート「{title}」を作成しますか？",
        "ko": "노트 “{title}”를 만들까요?", "id": "Buat catatan “{title}”?",
    },
    "page.note_editor.link_picker_empty": {
        "en": "No matching notes", "es": "Sin notas coincidentes",
        "pt-BR": "Nenhuma nota correspondente", "fr": "Aucune note correspondante",
        "de": "Keine passenden Notizen", "it": "Nessuna nota corrispondente",
        "ru": "Нет подходящих заметок", "tr": "Eşleşen not yok",
        "ar": "لا توجد ملاحظات مطابقة", "hi": "कोई मिलती नोट नहीं",
        "zh-CN": "没有匹配的笔记", "ja": "一致するノートがありません",
        "ko": "일치하는 노트가 없어요", "id": "Tidak ada catatan yang cocok",
    },
}

ROOT = pathlib.Path(__file__).resolve().parents[1] / "crates" / "i18n" / "locales"
LOCALES = ["en","es","pt-BR","fr","de","it","ru","tr","ar","hi","zh-CN","ja","ko","id"]
for code in LOCALES:
    path = ROOT / f"{code}.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    for key, by_locale in KEYS.items():
        data[key] = by_locale[code]
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print("done")
