// crates/messaging/src/i18n.rs

/// Get a translated string for the given key and language.
pub fn t<'a>(key: &'a str, lang: &str) -> &'a str {
    let l = match lang {
        "fr" => Lang::Fr,
        "es" => Lang::Es,
        "de" => Lang::De,
        "pt" => Lang::Pt,
        _ => Lang::En,
    };
    translate(key, l).unwrap_or(key)
}

/// Get a translated string with one argument substituted.
pub fn t1(key: &str, lang: &str, arg: &str) -> String {
    t(key, lang).replace("{}", arg)
}

#[derive(Clone, Copy)]
enum Lang { En, Fr, Es, De, Pt }

fn translate(key: &str, lang: Lang) -> Option<&'static str> {
    Some(match (key, lang) {
        // === Todo operations ===
        ("todo.added", Lang::En) => "todo added",
        ("todo.added", Lang::Fr) => "tâche ajoutée",
        ("todo.added", Lang::Es) => "tarea añadida",
        ("todo.added", Lang::De) => "Aufgabe hinzugefügt",
        ("todo.added", Lang::Pt) => "tarefa adicionada",

        ("todos.added", Lang::En) => "todos added",
        ("todos.added", Lang::Fr) => "tâches ajoutées",
        ("todos.added", Lang::Es) => "tareas añadidas",
        ("todos.added", Lang::De) => "Aufgaben hinzugefügt",
        ("todos.added", Lang::Pt) => "tarefas adicionadas",

        ("todo.done", Lang::En) => "done.",
        ("todo.done", Lang::Fr) => "fait.",
        ("todo.done", Lang::Es) => "hecho.",
        ("todo.done", Lang::De) => "erledigt.",
        ("todo.done", Lang::Pt) => "feito.",

        ("todo.deleted", Lang::En) => "deleted.",
        ("todo.deleted", Lang::Fr) => "supprimé.",
        ("todo.deleted", Lang::Es) => "eliminado.",
        ("todo.deleted", Lang::De) => "gelöscht.",
        ("todo.deleted", Lang::Pt) => "excluído.",

        ("todo.not_found", Lang::En) => "No todo #{}.",
        ("todo.not_found", Lang::Fr) => "Pas de tâche #{}.",
        ("todo.not_found", Lang::Es) => "No hay tarea #{}.",
        ("todo.not_found", Lang::De) => "Keine Aufgabe #{}.",
        ("todo.not_found", Lang::Pt) => "Nenhuma tarefa #{}.",

        ("todo.no_match", Lang::En) => "no match.",
        ("todo.no_match", Lang::Fr) => "aucune correspondance.",
        ("todo.no_match", Lang::Es) => "sin coincidencia.",
        ("todo.no_match", Lang::De) => "keine Übereinstimmung.",
        ("todo.no_match", Lang::Pt) => "sem correspondência.",

        ("todo.recurrence", Lang::En) => "Next occurrence created.",
        ("todo.recurrence", Lang::Fr) => "Prochaine occurrence créée.",
        ("todo.recurrence", Lang::Es) => "Próxima ocurrencia creada.",
        ("todo.recurrence", Lang::De) => "Nächstes Vorkommen erstellt.",
        ("todo.recurrence", Lang::Pt) => "Próxima ocorrência criada.",

        // === Notes ===
        ("note.saved", Lang::En) => "Note saved.",
        ("note.saved", Lang::Fr) => "Note enregistrée.",
        ("note.saved", Lang::Es) => "Nota guardada.",
        ("note.saved", Lang::De) => "Notiz gespeichert.",
        ("note.saved", Lang::Pt) => "Nota salva.",

        ("note.empty_content", Lang::En) => "No message content to save.",
        ("note.empty_content", Lang::Fr) => "Pas de contenu à sauvegarder.",
        ("note.empty_content", Lang::Es) => "Sin contenido para guardar.",
        ("note.empty_content", Lang::De) => "Kein Inhalt zum Speichern.",
        ("note.empty_content", Lang::Pt) => "Sem conteúdo para salvar.",

        // === Empty states ===
        ("empty.todos_open", Lang::En) => "Nothing to do. Suspicious.",
        ("empty.todos_open", Lang::Fr) => "Rien à faire. Suspect.",
        ("empty.todos_open", Lang::Es) => "Nada que hacer. Sospechoso.",
        ("empty.todos_open", Lang::De) => "Nichts zu tun. Verdächtig.",
        ("empty.todos_open", Lang::Pt) => "Nada para fazer. Suspeito.",

        ("empty.todos_done", Lang::En) => "Nothing done yet. Get to work.",
        ("empty.todos_done", Lang::Fr) => "Rien de fait. Au boulot.",
        ("empty.todos_done", Lang::Es) => "Nada hecho aún. A trabajar.",
        ("empty.todos_done", Lang::De) => "Noch nichts erledigt. An die Arbeit.",
        ("empty.todos_done", Lang::Pt) => "Nada feito ainda. Ao trabalho.",

        ("empty.todos_mine", Lang::En) => "Nothing assigned to you. Lucky.",
        ("empty.todos_mine", Lang::Fr) => "Rien pour toi. Veinard.",
        ("empty.todos_mine", Lang::Es) => "Nada asignado. Suerte.",
        ("empty.todos_mine", Lang::De) => "Nichts für dich. Glückspilz.",
        ("empty.todos_mine", Lang::Pt) => "Nada atribuído. Sortudo.",

        ("empty.notes", Lang::En) => "No notes. The group memory is blank.",
        ("empty.notes", Lang::Fr) => "Aucune note. La mémoire du groupe est vide.",
        ("empty.notes", Lang::Es) => "Sin notas. La memoria del grupo está vacía.",
        ("empty.notes", Lang::De) => "Keine Notizen. Das Gruppengedächtnis ist leer.",
        ("empty.notes", Lang::Pt) => "Sem notas. A memória do grupo está vazia.",

        ("empty.history", Lang::En) => "Nothing happened yet. Eerily quiet.",
        ("empty.history", Lang::Fr) => "Rien ne s'est passé. Étrangement calme.",
        ("empty.history", Lang::Es) => "Nada ha pasado. Inquietantemente tranquilo.",
        ("empty.history", Lang::De) => "Nichts passiert. Unheimlich still.",
        ("empty.history", Lang::Pt) => "Nada aconteceu. Estranhamente calmo.",

        // === Reminders ===
        ("reminder.set", Lang::En) => "Reminder set.",
        ("reminder.set", Lang::Fr) => "Rappel créé.",
        ("reminder.set", Lang::Es) => "Recordatorio creado.",
        ("reminder.set", Lang::De) => "Erinnerung erstellt.",
        ("reminder.set", Lang::Pt) => "Lembrete criado.",

        ("reminder.fired", Lang::En) => "Reminder:",
        ("reminder.fired", Lang::Fr) => "Rappel :",
        ("reminder.fired", Lang::Es) => "Recordatorio:",
        ("reminder.fired", Lang::De) => "Erinnerung:",
        ("reminder.fired", Lang::Pt) => "Lembrete:",

        // === Card replies ===
        ("reply.done", Lang::En) => "Done.",
        ("reply.done", Lang::Fr) => "Fait.",
        ("reply.done", Lang::Es) => "Hecho.",
        ("reply.done", Lang::De) => "Erledigt.",
        ("reply.done", Lang::Pt) => "Feito.",

        ("reply.deleted", Lang::En) => "Deleted.",
        ("reply.deleted", Lang::Fr) => "Supprimé.",
        ("reply.deleted", Lang::Es) => "Eliminado.",
        ("reply.deleted", Lang::De) => "Gelöscht.",
        ("reply.deleted", Lang::Pt) => "Excluído.",

        ("reply.snoozed", Lang::En) => "Snoozed to {}.",
        ("reply.snoozed", Lang::Fr) => "Reporté à {}.",
        ("reply.snoozed", Lang::Es) => "Pospuesto a {}.",
        ("reply.snoozed", Lang::De) => "Verschoben auf {}.",
        ("reply.snoozed", Lang::Pt) => "Adiado para {}.",

        ("reply.reassigned", Lang::En) => "Reassigned to @{}.",
        ("reply.reassigned", Lang::Fr) => "Réassigné à @{}.",
        ("reply.reassigned", Lang::Es) => "Reasignado a @{}.",
        ("reply.reassigned", Lang::De) => "Neu zugewiesen an @{}.",
        ("reply.reassigned", Lang::Pt) => "Reatribuído a @{}.",

        // === Task card ===
        ("card.reply_hint", Lang::En) => "Reply: done · snooze · edit · @reassign",
        ("card.reply_hint", Lang::Fr) => "Répondre : fait · reporter · edit · @réassigner",
        ("card.reply_hint", Lang::Es) => "Responder: hecho · posponer · editar · @reasignar",
        ("card.reply_hint", Lang::De) => "Antwort: erledigt · verschieben · edit · @zuweisen",
        ("card.reply_hint", Lang::Pt) => "Responder: feito · adiar · editar · @reatribuir",

        ("card.high_priority", Lang::En) => "High priority",
        ("card.high_priority", Lang::Fr) => "Haute priorité",
        ("card.high_priority", Lang::Es) => "Alta prioridad",
        ("card.high_priority", Lang::De) => "Hohe Priorität",
        ("card.high_priority", Lang::Pt) => "Alta prioridade",

        ("card.low_priority", Lang::En) => "Low priority",
        ("card.low_priority", Lang::Fr) => "Basse priorité",
        ("card.low_priority", Lang::Es) => "Baja prioridad",
        ("card.low_priority", Lang::De) => "Niedrige Priorität",
        ("card.low_priority", Lang::Pt) => "Baixa prioridade",

        // === Quota ===
        ("quota.todo_limit", Lang::En) => "Todo limit reached. Upgrade: grumps.io/billing",
        ("quota.todo_limit", Lang::Fr) => "Limite de tâches atteinte. Upgrade : grumps.io/billing",
        ("quota.todo_limit", Lang::Es) => "Límite de tareas alcanzado. Upgrade: grumps.io/billing",
        ("quota.todo_limit", Lang::De) => "Aufgabenlimit erreicht. Upgrade: grumps.io/billing",
        ("quota.todo_limit", Lang::Pt) => "Limite de tarefas atingido. Upgrade: grumps.io/billing",

        // === Fallback ===
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_default() {
        assert_eq!(t("reply.done", "en"), "Done.");
        assert_eq!(t("reply.done", "unknown"), "Done."); // defaults to en
    }

    #[test]
    fn french() {
        assert_eq!(t("reply.done", "fr"), "Fait.");
        assert_eq!(t("empty.todos_open", "fr"), "Rien à faire. Suspect.");
    }

    #[test]
    fn spanish() {
        assert_eq!(t("reply.done", "es"), "Hecho.");
    }

    #[test]
    fn german() {
        assert_eq!(t("reply.done", "de"), "Erledigt.");
    }

    #[test]
    fn portuguese() {
        assert_eq!(t("reply.done", "pt"), "Feito.");
    }

    #[test]
    fn substitution() {
        assert_eq!(t1("reply.snoozed", "en", "tomorrow"), "Snoozed to tomorrow.");
        assert_eq!(t1("reply.snoozed", "fr", "demain"), "Reporté à demain.");
    }

    #[test]
    fn unknown_key_returns_key() {
        assert_eq!(t("nonexistent.key", "en"), "nonexistent.key");
    }

    #[test]
    fn all_languages_have_reply_done() {
        for lang in ["en", "fr", "es", "de", "pt"] {
            assert_ne!(t("reply.done", lang), "reply.done", "missing translation for {}", lang);
        }
    }
}
