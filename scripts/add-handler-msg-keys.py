#!/usr/bin/env python3
"""Localize the most-used bot reply strings in handler.rs."""
import json, pathlib

KEYS = {
    "agent.todo.deleted": {
        "en": "🗑️ #{seq} \"{title}\" — deleted.",
        "es": "🗑️ #{seq} «{title}» — eliminado.",
        "pt-BR": "🗑️ #{seq} «{title}» — excluído.",
        "fr": "🗑️ #{seq} « {title} » — supprimée.",
        "de": "🗑️ #{seq} „{title}\" — gelöscht.",
        "it": "🗑️ #{seq} «{title}» — eliminato.",
        "ru": "🗑️ #{seq} «{title}» — удалено.",
        "tr": "🗑️ #{seq} «{title}» — silindi.",
        "ar": "🗑️ #{seq} «{title}» — تم الحذف.",
        "hi": "🗑️ #{seq} «{title}» — हटाया।",
        "zh-CN": "🗑️ #{seq} 「{title}」 — 已删除。",
        "ja": "🗑️ #{seq} 「{title}」 — 削除しました。",
        "ko": "🗑️ #{seq} 「{title}」 — 삭제됨.",
        "id": "🗑️ #{seq} «{title}» — dihapus.",
    },
    "agent.todo.not_found": {
        "en": "❓ No todo #{seq}.",
        "es": "❓ No hay tarea #{seq}.",
        "pt-BR": "❓ Não há tarefa #{seq}.",
        "fr": "❓ Aucune tâche #{seq}.",
        "de": "❓ Keine Aufgabe #{seq}.",
        "it": "❓ Nessun todo #{seq}.",
        "ru": "❓ Нет задачи #{seq}.",
        "tr": "❓ #{seq} numaralı görev yok.",
        "ar": "❓ لا توجد مهمة #{seq}.",
        "hi": "❓ कोई टूडू #{seq} नहीं।",
        "zh-CN": "❓ 没有待办 #{seq}。",
        "ja": "❓ Todo #{seq} は見つかりません。",
        "ko": "❓ Todo #{seq}을(를) 찾을 수 없어요.",
        "id": "❓ Tidak ada todo #{seq}.",
    },
    "agent.note.saved": {
        "en": "📝 Note{title} saved.",
        "es": "📝 Nota{title} guardada.",
        "pt-BR": "📝 Nota{title} salva.",
        "fr": "📝 Note{title} enregistrée.",
        "de": "📝 Notiz{title} gespeichert.",
        "it": "📝 Nota{title} salvata.",
        "ru": "📝 Заметка{title} сохранена.",
        "tr": "📝 Not{title} kaydedildi.",
        "ar": "📝 الملاحظة{title} محفوظة.",
        "hi": "📝 नोट{title} सहेज लिया।",
        "zh-CN": "📝 便签{title} 已保存。",
        "ja": "📝 ノート{title} を保存しました。",
        "ko": "📝 노트{title} 저장됨.",
        "id": "📝 Catatan{title} disimpan.",
    },
    "agent.quoted.no_todo": {
        "en": "❓ No message content to turn into a todo.",
        "es": "❓ No hay contenido en el mensaje para convertir en tarea.",
        "pt-BR": "❓ Sem conteúdo no mensagem para virar tarefa.",
        "fr": "❓ Aucun contenu à transformer en tâche.",
        "de": "❓ Keine Nachricht zum Umwandeln in eine Aufgabe.",
        "it": "❓ Nessun contenuto da trasformare in todo.",
        "ru": "❓ Нет содержимого для превращения в задачу.",
        "tr": "❓ Göreve dönüştürülecek içerik yok.",
        "ar": "❓ لا يوجد محتوى لتحويله إلى مهمة.",
        "hi": "❓ टूडू बनाने के लिए कोई सामग्री नहीं।",
        "zh-CN": "❓ 没有可转换为待办的消息内容。",
        "ja": "❓ Todo にするメッセージ内容がありません。",
        "ko": "❓ Todo로 만들 메시지 내용이 없어요.",
        "id": "❓ Tidak ada isi pesan untuk dijadikan todo.",
    },
    "agent.quoted.no_note": {
        "en": "❓ No message content to save as a note.",
        "es": "❓ No hay contenido en el mensaje para guardar como nota.",
        "pt-BR": "❓ Sem conteúdo no mensagem para salvar como nota.",
        "fr": "❓ Aucun contenu à enregistrer en note.",
        "de": "❓ Keine Nachricht zum Speichern als Notiz.",
        "it": "❓ Nessun contenuto da salvare come nota.",
        "ru": "❓ Нет содержимого для сохранения в заметку.",
        "tr": "❓ Not olarak kaydedilecek içerik yok.",
        "ar": "❓ لا يوجد محتوى لحفظه كملاحظة.",
        "hi": "❓ नोट के रूप में सहेजने के लिए कोई सामग्री नहीं।",
        "zh-CN": "❓ 没有可保存为便签的消息内容。",
        "ja": "❓ ノートとして保存するメッセージ内容がありません。",
        "ko": "❓ 노트로 저장할 메시지 내용이 없어요.",
        "id": "❓ Tidak ada isi pesan untuk disimpan sebagai catatan.",
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
