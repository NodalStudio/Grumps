#!/usr/bin/env python3
"""Add localized strings for handler.rs replacements."""
import json, pathlib

KEYS = {
    "agent.files.web_only": {
        "en": "📁 File listing is on the web workspace.",
        "es": "📁 La lista de archivos está en el espacio web.",
        "pt-BR": "📁 A lista de arquivos está no espaço web.",
        "fr": "📁 La liste des fichiers est sur l'espace web.",
        "de": "📁 Dateiliste im Web-Workspace.",
        "it": "📁 L'elenco file è nello spazio web.",
        "ru": "📁 Список файлов в веб-рабочей области.",
        "tr": "📁 Dosya listesi web çalışma alanında.",
        "ar": "📁 قائمة الملفات في مساحة العمل عبر الويب.",
        "hi": "📁 फाइल सूची वेब कार्यक्षेत्र में है।",
        "zh-CN": "📁 文件列表在网页工作区。",
        "ja": "📁 ファイル一覧はウェブのワークスペースにあります。",
        "ko": "📁 파일 목록은 웹 워크스페이스에 있어요.",
        "id": "📁 Daftar file ada di ruang kerja web.",
    },
    "agent.notes.search_empty": {
        "en": "🔍 No notes matching \"{query}\".",
        "es": "🔍 No hay notas que coincidan con «{query}».",
        "pt-BR": "🔍 Nenhuma nota corresponde a «{query}».",
        "fr": "🔍 Aucune note ne correspond à « {query} ».",
        "de": "🔍 Keine Notizen passen zu „{query}\".",
        "it": "🔍 Nessuna nota corrisponde a «{query}».",
        "ru": "🔍 Нет заметок по запросу «{query}».",
        "tr": "🔍 «{query}» ile eşleşen not yok.",
        "ar": "🔍 لا توجد ملاحظات تطابق «{query}».",
        "hi": "🔍 «{query}» से मेल खाते कोई नोट नहीं।",
        "zh-CN": "🔍 没有匹配 \"{query}\" 的便签。",
        "ja": "🔍 「{query}」に一致するノートはありません。",
        "ko": "🔍 \"{query}\"와 일치하는 노트가 없어요.",
        "id": "🔍 Tidak ada catatan cocok dengan \"{query}\".",
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
