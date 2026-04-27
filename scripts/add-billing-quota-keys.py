#!/usr/bin/env python3
"""Add localized quota-exceeded messages for the billing module."""
import json, pathlib

KEYS = {
    "billing.quota.todos": {
        "en": "Todo limit reached ({current}/{max}). Upgrade to add more: grumps.app/billing",
        "es": "Límite de tareas alcanzado ({current}/{max}). Actualiza para añadir más: grumps.app/billing",
        "pt-BR": "Limite de tarefas atingido ({current}/{max}). Atualize para adicionar mais: grumps.app/billing",
        "fr": "Limite de tâches atteinte ({current}/{max}). Passe au plan supérieur : grumps.app/billing",
        "de": "Aufgaben-Limit erreicht ({current}/{max}). Upgrade: grumps.app/billing",
        "it": "Limite todo raggiunto ({current}/{max}). Esegui l'upgrade: grumps.app/billing",
        "ru": "Достигнут лимит задач ({current}/{max}). Обновите план: grumps.app/billing",
        "tr": "Görev limiti aşıldı ({current}/{max}). Yükselt: grumps.app/billing",
        "ar": "تم بلوغ حد المهام ({current}/{max}). الترقية: grumps.app/billing",
        "hi": "टूडू सीमा पूरी ({current}/{max}). अपग्रेड: grumps.app/billing",
        "zh-CN": "已达待办上限 ({current}/{max})。升级: grumps.app/billing",
        "ja": "Todo の上限に達しました ({current}/{max})。アップグレード: grumps.app/billing",
        "ko": "할 일 한도 도달 ({current}/{max}). 업그레이드: grumps.app/billing",
        "id": "Batas todo tercapai ({current}/{max}). Upgrade: grumps.app/billing",
    },
    "billing.quota.notes": {
        "en": "Note limit reached ({current}/{max}). Upgrade: grumps.app/billing",
        "es": "Límite de notas alcanzado ({current}/{max}). Actualiza: grumps.app/billing",
        "pt-BR": "Limite de notas atingido ({current}/{max}). Atualize: grumps.app/billing",
        "fr": "Limite de notes atteinte ({current}/{max}). Passe au plan supérieur : grumps.app/billing",
        "de": "Notizen-Limit erreicht ({current}/{max}). Upgrade: grumps.app/billing",
        "it": "Limite note raggiunto ({current}/{max}). Esegui l'upgrade: grumps.app/billing",
        "ru": "Достигнут лимит заметок ({current}/{max}). Обновите план: grumps.app/billing",
        "tr": "Not limiti aşıldı ({current}/{max}). Yükselt: grumps.app/billing",
        "ar": "تم بلوغ حد الملاحظات ({current}/{max}). الترقية: grumps.app/billing",
        "hi": "नोट्स सीमा पूरी ({current}/{max}). अपग्रेड: grumps.app/billing",
        "zh-CN": "已达便签上限 ({current}/{max})。升级: grumps.app/billing",
        "ja": "ノートの上限に達しました ({current}/{max})。アップグレード: grumps.app/billing",
        "ko": "노트 한도 도달 ({current}/{max}). 업그레이드: grumps.app/billing",
        "id": "Batas catatan tercapai ({current}/{max}). Upgrade: grumps.app/billing",
    },
    "billing.quota.llm": {
        "en": "AI message limit reached ({current}/{max}). Upgrade: grumps.app/billing",
        "es": "Límite de mensajes IA alcanzado ({current}/{max}). Actualiza: grumps.app/billing",
        "pt-BR": "Limite de mensagens IA atingido ({current}/{max}). Atualize: grumps.app/billing",
        "fr": "Limite de messages IA atteinte ({current}/{max}). Passe au plan supérieur : grumps.app/billing",
        "de": "KI-Nachrichten-Limit erreicht ({current}/{max}). Upgrade: grumps.app/billing",
        "it": "Limite messaggi IA raggiunto ({current}/{max}). Esegui l'upgrade: grumps.app/billing",
        "ru": "Достигнут лимит сообщений ИИ ({current}/{max}). Обновите план: grumps.app/billing",
        "tr": "AI mesaj limiti aşıldı ({current}/{max}). Yükselt: grumps.app/billing",
        "ar": "تم بلوغ حد رسائل الذكاء الاصطناعي ({current}/{max}). الترقية: grumps.app/billing",
        "hi": "एआई संदेश सीमा पूरी ({current}/{max}). अपग्रेड: grumps.app/billing",
        "zh-CN": "已达 AI 消息上限 ({current}/{max})。升级: grumps.app/billing",
        "ja": "AI メッセージの上限に達しました ({current}/{max})。アップグレード: grumps.app/billing",
        "ko": "AI 메시지 한도 도달 ({current}/{max}). 업그레이드: grumps.app/billing",
        "id": "Batas pesan AI tercapai ({current}/{max}). Upgrade: grumps.app/billing",
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
    print(f"updated {path.name}")
