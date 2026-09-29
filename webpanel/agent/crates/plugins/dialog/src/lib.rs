//! Cale de `tauri-plugin-dialog`.
//!
//! Seul usage cote Rust dans NiTriTe : la confirmation NATIVE exigee avant
//! d'executer une commande d'administration dans le Terminal (`run_in_shell`).
//! Elle doit rester hors de la page web — c'est tout son interet : un script
//! injecte dans la page ne peut pas cliquer une boite de dialogue Windows.
//! L'agent tourne sur le poste de l'utilisateur, la boite s'affiche donc sur
//! son bureau, exactement comme dans l'application native.

pub struct Plugin;

pub fn init() -> Plugin {
    Plugin
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageDialogKind {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageDialogButtons {
    Ok,
    OkCancel,
    YesNo,
    OkCustom(String),
    OkCancelCustom(String, String),
}

pub struct Dialog;

pub trait DialogExt {
    fn dialog(&self) -> Dialog;
}

impl DialogExt for tauri::AppHandle {
    fn dialog(&self) -> Dialog {
        Dialog
    }
}

impl DialogExt for tauri::Window {
    fn dialog(&self) -> Dialog {
        Dialog
    }
}

impl Dialog {
    pub fn message(&self, text: impl Into<String>) -> MessageDialogBuilder {
        MessageDialogBuilder {
            text: text.into(),
            title: String::from("NiTriTe"),
            kind: MessageDialogKind::Info,
            buttons: MessageDialogButtons::Ok,
        }
    }
}

pub struct MessageDialogBuilder {
    text: String,
    title: String,
    kind: MessageDialogKind,
    buttons: MessageDialogButtons,
}

impl MessageDialogBuilder {
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn kind(mut self, kind: MessageDialogKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn buttons(mut self, buttons: MessageDialogButtons) -> Self {
        self.buttons = buttons;
        self
    }

    /// Bloque jusqu'a la reponse. `true` = OK / Oui / bouton de validation.
    pub fn blocking_show(self) -> bool {
        let level = match self.kind {
            MessageDialogKind::Info => rfd::MessageLevel::Info,
            MessageDialogKind::Warning => rfd::MessageLevel::Warning,
            MessageDialogKind::Error => rfd::MessageLevel::Error,
        };
        let buttons = match &self.buttons {
            MessageDialogButtons::Ok => rfd::MessageButtons::Ok,
            MessageDialogButtons::OkCancel => rfd::MessageButtons::OkCancel,
            MessageDialogButtons::YesNo => rfd::MessageButtons::YesNo,
            MessageDialogButtons::OkCustom(ok) => rfd::MessageButtons::OkCustom(ok.clone()),
            MessageDialogButtons::OkCancelCustom(ok, cancel) => {
                rfd::MessageButtons::OkCancelCustom(ok.clone(), cancel.clone())
            }
        };
        let accept = match &self.buttons {
            MessageDialogButtons::OkCustom(ok) | MessageDialogButtons::OkCancelCustom(ok, _) => Some(ok.clone()),
            _ => None,
        };
        let result = rfd::MessageDialog::new()
            .set_title(&self.title)
            .set_description(&self.text)
            .set_level(level)
            .set_buttons(buttons)
            .show();
        match result {
            rfd::MessageDialogResult::Ok | rfd::MessageDialogResult::Yes => true,
            rfd::MessageDialogResult::Custom(label) => accept.as_deref() == Some(label.as_str()),
            _ => false,
        }
    }
}
