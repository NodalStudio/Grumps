#!/usr/bin/env python3
"""Inject i18n keys for the agent fast-path commands (silence, unsilence,
memory save/error, generic technical fallback) into every locale JSON."""
import json, pathlib

KEYS = {
    "agent.silence.confirm": {
        "en":    "🤫 Quiet for 24h.",
        "es":    "🤫 Silencio durante 24h.",
        "pt-BR": "🤫 Silêncio por 24h.",
        "fr":    "🤫 Silence pour 24h.",
        "de":    "🤫 Ruhe für 24 Std.",
        "it":    "🤫 Silenzio per 24h.",
        "ru":    "🤫 Тишина на 24 ч.",
        "tr":    "🤫 24 saat sessizlik.",
        "ar":    "🤫 صمت لمدة 24 ساعة.",
        "hi":    "🤫 24 घंटे शांत।",
        "zh-CN": "🤫 安静 24 小时。",
        "ja":    "🤫 24時間静かにします。",
        "ko":    "🤫 24시간 조용히 할게요.",
        "id":    "🤫 Diam selama 24 jam.",
    },
    "agent.silence.unset": {
        "en":    "👋 I'm back.",
        "es":    "👋 He vuelto.",
        "pt-BR": "👋 Voltei.",
        "fr":    "👋 De retour.",
        "de":    "👋 Bin zurück.",
        "it":    "👋 Sono tornato.",
        "ru":    "👋 Я вернулся.",
        "tr":    "👋 Geri döndüm.",
        "ar":    "👋 عُدت.",
        "hi":    "👋 मैं वापस आ गया।",
        "zh-CN": "👋 我回来了。",
        "ja":    "👋 戻りました。",
        "ko":    "👋 돌아왔어요.",
        "id":    "👋 Aku kembali.",
    },
    "agent.memory.saved": {
        "en":    "💾 Got it.",
        "es":    "💾 Anotado.",
        "pt-BR": "💾 Anotado.",
        "fr":    "💾 Noté !",
        "de":    "💾 Gespeichert.",
        "it":    "💾 Annotato.",
        "ru":    "💾 Запомнил.",
        "tr":    "💾 Not aldım.",
        "ar":    "💾 سُجِّل.",
        "hi":    "💾 नोट कर लिया।",
        "zh-CN": "💾 记下了。",
        "ja":    "💾 メモしました。",
        "ko":    "💾 기억해 둘게요.",
        "id":    "💾 Dicatat.",
    },
    "agent.memory.error": {
        "en":    "Couldn't save. Try again?",
        "es":    "No pude guardar. ¿Reintentar?",
        "pt-BR": "Não consegui salvar. Tentar de novo?",
        "fr":    "Impossible de sauvegarder. Réessaie ?",
        "de":    "Konnte nicht speichern. Erneut versuchen?",
        "it":    "Non sono riuscito a salvare. Riprovare?",
        "ru":    "Не удалось сохранить. Попробовать ещё раз?",
        "tr":    "Kaydedemedim. Tekrar dener misin?",
        "ar":    "لم أستطع الحفظ. حاول مجددًا؟",
        "hi":    "सहेज नहीं पाया। फिर कोशिश करें?",
        "zh-CN": "保存失败。重试？",
        "ja":    "保存できませんでした。もう一度試しますか？",
        "ko":    "저장하지 못했어요. 다시 시도할까요?",
        "id":    "Tidak bisa simpan. Coba lagi?",
    },
    "agent.error.technical": {
        "en":    "Sorry, technical hiccup. Try again?",
        "es":    "Lo siento, fallo técnico. ¿Reintentar?",
        "pt-BR": "Desculpe, falha técnica. Tentar de novo?",
        "fr":    "Désolé, j'ai eu un souci technique. Réessaie ?",
        "de":    "Tut mir leid, technisches Problem. Erneut versuchen?",
        "it":    "Scusa, problema tecnico. Riprovare?",
        "ru":    "Извини, техническая ошибка. Попробовать ещё раз?",
        "tr":    "Üzgünüm, teknik aksaklık. Tekrar dener misin?",
        "ar":    "آسف، عطل تقني. حاول مجددًا؟",
        "hi":    "माफ कीजिए, तकनीकी दिक्कत। फिर कोशिश करें?",
        "zh-CN": "抱歉,出了点技术问题。重试?",
        "ja":    "技術的な問題が起きました。もう一度試しますか？",
        "ko":    "기술적 문제가 있었어요. 다시 시도할까요?",
        "id":    "Maaf, masalah teknis. Coba lagi?",
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
