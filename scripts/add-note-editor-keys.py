#!/usr/bin/env python3
"""Localize note-editor UI strings."""
import json, pathlib

KEYS = {
    "page.note_editor.untitled": {
        "en": "(untitled)", "es": "(sin título)", "pt-BR": "(sem título)",
        "fr": "(sans titre)", "de": "(ohne Titel)", "it": "(senza titolo)",
        "ru": "(без названия)", "tr": "(başlıksız)", "ar": "(بدون عنوان)",
        "hi": "(शीर्षकहीन)", "zh-CN": "(无标题)", "ja": "(無題)",
        "ko": "(제목 없음)", "id": "(tanpa judul)",
    },
    "page.note_editor.created": {
        "en": "Created {date}", "es": "Creado {date}", "pt-BR": "Criado em {date}",
        "fr": "Créé le {date}", "de": "Erstellt am {date}", "it": "Creato il {date}",
        "ru": "Создано {date}", "tr": "Oluşturuldu {date}", "ar": "أُنشئ في {date}",
        "hi": "{date} को बनाया", "zh-CN": "创建于 {date}", "ja": "作成日 {date}",
        "ko": "생성일 {date}", "id": "Dibuat {date}",
    },
    "page.note_editor.edit": {
        "en": "Edit", "es": "Editar", "pt-BR": "Editar", "fr": "Modifier",
        "de": "Bearbeiten", "it": "Modifica", "ru": "Изменить", "tr": "Düzenle",
        "ar": "تعديل", "hi": "संपादित", "zh-CN": "编辑", "ja": "編集",
        "ko": "편집", "id": "Edit",
    },
    "page.note_editor.preview": {
        "en": "Preview", "es": "Vista", "pt-BR": "Visualizar", "fr": "Aperçu",
        "de": "Vorschau", "it": "Anteprima", "ru": "Предпросмотр", "tr": "Önizleme",
        "ar": "معاينة", "hi": "पूर्वावलोकन", "zh-CN": "预览", "ja": "プレビュー",
        "ko": "미리보기", "id": "Pratinjau",
    },
    "page.note_editor.save": {
        "en": "Save", "es": "Guardar", "pt-BR": "Salvar", "fr": "Enregistrer",
        "de": "Speichern", "it": "Salva", "ru": "Сохранить", "tr": "Kaydet",
        "ar": "حفظ", "hi": "सहेजें", "zh-CN": "保存", "ja": "保存",
        "ko": "저장", "id": "Simpan",
    },
    "page.note_editor.saving": {
        "en": "Saving…", "es": "Guardando…", "pt-BR": "Salvando…", "fr": "Enregistrement…",
        "de": "Speichern…", "it": "Salvataggio…", "ru": "Сохранение…", "tr": "Kaydediliyor…",
        "ar": "جارٍ الحفظ…", "hi": "सहेज रहे हैं…", "zh-CN": "保存中…", "ja": "保存中…",
        "ko": "저장 중…", "id": "Menyimpan…",
    },
    "page.note_editor.saved": {
        "en": "Saved.", "es": "Guardado.", "pt-BR": "Salvo.", "fr": "Enregistré.",
        "de": "Gespeichert.", "it": "Salvato.", "ru": "Сохранено.", "tr": "Kaydedildi.",
        "ar": "تم الحفظ.", "hi": "सहेज लिया।", "zh-CN": "已保存。", "ja": "保存しました。",
        "ko": "저장됨.", "id": "Disimpan.",
    },
    "page.note_editor.save_error": {
        "en": "Save failed. Try again?", "es": "Error al guardar. ¿Reintentar?",
        "pt-BR": "Falha ao salvar. Tentar de novo?", "fr": "Échec de l'enregistrement. Réessayer ?",
        "de": "Speichern fehlgeschlagen. Erneut versuchen?", "it": "Salvataggio fallito. Riprovare?",
        "ru": "Ошибка сохранения. Попробовать ещё раз?", "tr": "Kaydedilemedi. Tekrar dener misin?",
        "ar": "فشل الحفظ. حاول مجددًا؟", "hi": "सहेज नहीं पाया। फिर कोशिश करें?",
        "zh-CN": "保存失败。重试?", "ja": "保存できませんでした。もう一度試しますか？",
        "ko": "저장하지 못했어요. 다시 시도할까요?", "id": "Gagal menyimpan. Coba lagi?",
    },
    "page.note_editor.not_found": {
        "en": "Note not found.", "es": "Nota no encontrada.", "pt-BR": "Nota não encontrada.",
        "fr": "Note introuvable.", "de": "Notiz nicht gefunden.", "it": "Nota non trovata.",
        "ru": "Заметка не найдена.", "tr": "Not bulunamadı.", "ar": "الملاحظة غير موجودة.",
        "hi": "नोट नहीं मिला।", "zh-CN": "未找到便签。", "ja": "ノートが見つかりません。",
        "ko": "노트를 찾을 수 없어요.", "id": "Catatan tidak ditemukan.",
    },
    "common.title.placeholder": {
        "en": "Title", "es": "Título", "pt-BR": "Título", "fr": "Titre",
        "de": "Titel", "it": "Titolo", "ru": "Заголовок", "tr": "Başlık",
        "ar": "العنوان", "hi": "शीर्षक", "zh-CN": "标题", "ja": "タイトル",
        "ko": "제목", "id": "Judul",
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
